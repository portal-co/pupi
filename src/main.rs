use once_cell::sync::OnceCell;
use std::{
    // cell::OnceCell,
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{ErrorKind, Write, stderr, stdout},
    process::Command,
    sync::{Mutex, RwLock},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
fn out(c: &mut Command) -> std::io::Result<()> {
    let o = c.output()?;
    stdout().write_all(&o.stdout)?;
    stderr().write_all(&o.stderr)?;
    if !o.status.success() {
        eprintln!(
            "[FAIL] {} ({})@{}",
            c.get_program().display(),
            c.get_args()
                .map(|a| format!("[{}]", a.display()))
                .collect::<Vec<_>>()
                .join(" "),
            c.get_current_dir()
                .map(|d| format!("{}", d.display()))
                .unwrap_or_else(|| format!("[[current directory]]"))
        );
        std::process::exit(o.status.code().unwrap());
    }
    return Ok(());
}
fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    args.next();
    let cmd = args.next().unwrap();
    match &*cmd {
        "setup" => {
            let root_path = args.next().unwrap();
            if !std::fs::exists(format!("{root_path}/.git"))? {
                std::process::Command::new("git")
                    .arg("init")
                    .current_dir(&root_path)
                    .spawn()?
                    .wait()?;
                std::fs::write(
                    format!("{root_path}/.gitignore"),
                    r#"
                /target
                node_modules
                .parcel-cache
                "#,
                )?;
            }
            if !std::fs::exists(format!("{root_path}/pupi.json"))? {
                std::fs::write(format!("{root_path}/pupi.json"), r#"{}"#)?;
            }
            if !std::fs::exists(format!("{root_path}/package.json"))? {
                std::fs::write(
                    format!("{root_path}/package.json"),
                    r#"{"name":"temp","workspaces":[]}"#,
                )?;
            }
            if !std::fs::exists(format!("{root_path}/Cargo.toml"))? {
                std::fs::write(
                    format!("{root_path}/Cargo.toml"),
                    r#"
                    [workspace]
                    members=[]
                    resolver="3"
                    [workspace.package]
                    [workspace.dependencies]
                    "#,
                )?;
            }
            std::process::Command::new("npm")
                .arg("install")
                .arg("--save-dev")
                .arg("parcel")
                .arg("zshy")
                .arg("typescript")
                .arg("@parcel/packager-ts")
                .arg("@parcel/transformer-typescript-types")
                .current_dir(&root_path)
                .spawn()?
                .wait()?;
        }
        // "subtree" => {
        //     let root_path = args.next().unwrap();
        //     let args = args.collect::<Vec<_>>();
        //     let root: Root =
        //         serde_json::from_reader(File::open(format!("{root_path}/pupi.json"))?)?;
        //     // let visited = RwLock::new(BTreeSet::new());
        //     let mut error = OnceCell::new();
        //     let roots: Mutex<BTreeMap<String, Root>> = Mutex::new(BTreeMap::new());
        //     std::thread::scope(|s| {
        //         for (path, member) in root.members.iter() {
        //             // });
        //         }
        //     });
        //     if let Some(e) = error.take() {
        //         return Err(e);
        //     }
        //     let mut root = root;
        //     for (k, r) in roots.into_inner().unwrap() {
        //         for (p, m) in r.members {
        //             root.members.insert(format!("{k}/{p}").replace("./", ""), m);
        //         }
        //     }
        //     std::fs::write(
        //         format!("{root_path}/pupi.json"),
        //         serde_json::to_vec_pretty(&root)?,
        //     )?;
        // }
        _ => {
            let root_path = args.next().unwrap();
            let args = args.collect::<Vec<_>>();
            let root: Root =
                serde_json::from_reader(File::open(format!("{root_path}/pupi.json"))?)?;
            let visited = RwLock::new(BTreeSet::new());
            let mut error = OnceCell::new();
            let d = DepMap::default();
            add_workspaces(&root, &root_path)?;
            std::thread::scope(|s| {
                for (path, member) in root.members.iter() {
                    // let path = format!("{root_path}/{path}");
                    s.spawn(|| {
                        match update(
                            path,
                            &root_path,
                            member,
                            &root,
                            &visited,
                            &d,
                            &[cmd.clone()]
                                .into_iter()
                                .chain(args.clone())
                                .collect::<Vec<_>>(),
                        ) {
                            Ok(_) => {}
                            Err(e) => {
                                error.set(e);
                            }
                        }
                    });
                }
            });
            if let Some(e) = error.take() {
                return Err(e);
            }
        }
    }
    Ok(())
}
fn add_workspaces(root: &Root, root_path: &str) -> std::io::Result<()> {
    if std::fs::exists(format!("{root_path}/package.json"))? {
        let mut val: serde_json::Value =
            serde_json::from_reader(File::open(format!("{root_path}/package.json"))?)?;
        if let Some(o) = val.as_object_mut() {
            let w = o.get("workspaces").and_then(|a| a.as_array());
            o.insert(
                "workspaces".to_owned(),
                serde_json::Value::Array(
                    root.members
                        .iter()
                        .filter_map(|(a, b)| match b.npm.as_ref() {
                            None => None,
                            Some(_) => Some(a.clone()),
                        })
                        .chain(
                            w.iter()
                                .flat_map(|a| a.iter())
                                .filter_map(|a| a.as_str())
                                .map(|b| b.to_owned()),
                        )
                        .map(|mut a| {
                            while let Some(b) = a.strip_prefix("./") {
                                a = b.to_owned();
                            }
                            return a;
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .map(|a| serde_json::Value::String(a))
                        .collect(),
                ),
            );
        }
        std::fs::write(
            format!("{root_path}/package.json"),
            serde_json::to_vec_pretty(&val)?,
        )?;
    }
    if std::fs::exists(format!("{root_path}/Cargo.toml"))? {
        let mut val: toml::Table = std::fs::read_to_string(format!("{root_path}/Cargo.toml"))?
            .parse()
            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
        if let Some(m) = val
            .get_mut("workspace")
            .and_then(|a| a.as_table_mut())
            .and_then(|a| a.get_mut("members"))
            .and_then(|a| a.as_array_mut())
        {
            *m = root
                .members
                .iter()
                .filter_map(|(a, b)| match b.cargo.as_ref() {
                    None => None,
                    Some(_) => Some(a.clone()),
                })
                .chain(
                    m.into_iter()
                        .filter_map(|a| a.as_str().map(|a| a.to_owned())),
                )
                .map(|mut a| {
                    while let Some(b) = a.strip_prefix("./") {
                        a = b.to_owned();
                    }
                    return a;
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|a| toml::Value::String(a))
                .collect();
        }
        std::fs::write(
            format!("{root_path}/Cargo.toml"),
            toml::to_string_pretty(&val).map_err(|e| std::io::Error::new(ErrorKind::Other, e))?,
        )?;
    }
    Ok(())
}
#[derive(Default)]
struct DepMap {
    npm: OnceCell<BTreeMap<String, String>>,
    rnpm: OnceCell<BTreeMap<String, String>>,
    subroots: OnceCell<BTreeMap<String, (OnceCell<Root>, RwLock<BTreeSet<String>>, DepMap)>>,
}
impl DepMap {
    fn subroot(
        &self,
        root: &Root,
        root_path: &str,
        name: &str,
    ) -> std::io::Result<Option<(&Root, &RwLock<BTreeSet<String>>, String, &DepMap)>> {
        let m = self.subroots.get_or_try_init(|| {
            Ok::<_, std::io::Error>(
                root.members
                    .iter()
                    .flat_map(|(a, b)| {
                        b.subtree.iter().flat_map(move |a2| {
                            a2.paths.iter().map(move |(p, _)| format!("{a}/{p}"))
                        })
                    })
                    .map(|a| (a, Default::default()))
                    .collect(),
            )
        })?;
        let Some((m, r, n)) = m.get(name) else {
            return Ok(None);
        };
        let m = m.get_or_try_init(|| {
            let root: Root =
                serde_json::from_reader(File::open(format!("{root_path}/{name}/pupi.json"))?)?;
            Ok::<_, std::io::Error>(root)
        })?;
        return Ok(Some((m, r, format!("{root_path}/{name}"), n)));
    }
    fn npm(&self, root: &Root, root_path: &str) -> std::io::Result<&BTreeMap<String, String>> {
        return self.npm.get_or_try_init(|| {
            let mut m: BTreeMap<String, String> = BTreeMap::new();
            for (a, b) in root.members.iter() {
                if let Some(_) = b.npm.as_ref() {
                    let mut val: serde_json::Value = serde_json::from_reader(File::open(
                        format!("{root_path}/{a}/package.json"),
                    )?)?;
                    let name = val
                        .as_object()
                        .unwrap()
                        .get("name")
                        .unwrap()
                        .as_str()
                        .unwrap();
                    m.insert(a.clone(), name.to_owned());
                }
            }
            return Ok(m);
        });
    }
    fn rnpm(&self, root: &Root, root_path: &str) -> std::io::Result<&BTreeMap<String, String>> {
        return self.rnpm.get_or_try_init(|| {
            Ok(self
                .npm(root, root_path)?
                .iter()
                .map(|(a, b)| (b.clone(), a.clone()))
                .collect())
        });
    }
}
fn update_dep(
    xpath: &str,
    dep: &Dep,
    root_path: &str,
    member: &Member,
    root: &Root,
    visited: &RwLock<BTreeSet<String>>,
    depmap: &DepMap,
    cmd: &[String],
) -> std::io::Result<()> {
    let mut root_path = Cow::Borrowed(root_path);
    let mut root = root;
    let mut visited = visited;
    let mut depmap = depmap;
    let mut dep = dep;
    let do_update = matches!(&*cmd[0], "autogen" | "build" | "publish" | "update");
    loop {
        if let Some(s) = dep.subtree.as_ref() {
            if do_update {
                return update(xpath, &root_path, member, root, visited, depmap, cmd);
            }
            update_dep(
                &s.pkg_name,
                &s.pkg,
                &root_path,
                member,
                root,
                visited,
                depmap,
                cmd,
            )?;
            if let Some((a, b, c, d)) =
                depmap.subroot(root, &root_path, &format!("{}/{}", &s.pkg_name, &s.subtree))?
            {
                root_path = Cow::Owned(c);
                root = a;
                visited = b;
                depmap = d;
                dep = &s.nest;
                continue;
            }
        }
         return update(xpath, &root_path, member, root, visited, depmap, cmd);
    }
}
fn update(
    xpath: &str,
    root_path: &str,
    member: &Member,
    root: &Root,
    visited: &RwLock<BTreeSet<String>>,
    depmap: &DepMap,
    cmd: &[String],
) -> std::io::Result<()> {
    if visited.read().unwrap().contains(xpath) {
        return Ok(());
    }
    match visited.write().unwrap() {
        mut w => {
            // let mut w = ;
            if w.contains(xpath) {
                return Ok(());
            }
            w.insert(xpath.to_owned());
        }
    };
    let mut error = OnceCell::new();
    std::thread::scope(|s| {
        if let Some(subtree) = member.subtree.as_ref() {
            // s.spawn(||{}
            // std::thread::scope(|s| {
            for (p, v) in subtree.paths.iter().map(|(p, v)| (p.clone(), v.clone())) {
                let error = &error;
                let path = &xpath;
                let root_path = &root_path;
                // let roots = &roots;
                // let xpath = &xpath;
                s.spawn(move || {
                    match (move || {
                        out(std::process::Command::new("git")
                            .arg("subtree")
                            .arg("pull")
                            .arg("-P")
                            .arg(format!("{root_path}/{path}/{p}"))
                            .arg(v))?;
                        // if std::fs::exists(format!("{root_path}/{path}/{p}/pupi.json"))?
                        // {
                        //     let root2: Root = serde_json::from_reader(File::open(
                        //         format!("{root_path}/{path}/{p}/pupi.json"),
                        //     )?)?;
                        //     roots.lock().unwrap().insert(format!("{path}/{p}"), root2);
                        // }
                        Ok::<_, std::io::Error>(())
                    })() {
                        Ok(_) => {}
                        Err(e) => {
                            error.set(e);
                        }
                    }
                });
            }
        }
    });
    if let Some(e) = error.take() {
        return Err(e);
    }
    std::thread::scope(|s| {
        for (dep, x) in member.deps.iter() {
            let error = &error;
            s.spawn(move || {
                match update_dep(
                    &dep,
                    &x,
                    root_path,
                    root.members.get(dep).unwrap(),
                    root,
                    visited,
                    depmap,
                    cmd,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        error.set(e);
                    }
                }
            });
        }
    });
    if let Some(e) = error.take() {
        return Err(e);
    }
    eprintln!("[Build] Building {xpath}");
    let path = format!("{root_path}/{xpath}");
    let update = matches!(&*cmd[0], "autogen" | "build" | "publish" | "update");
    if let Some(u) = member.updater.as_ref() {
        match &*cmd[0] {
            "autogen" | "build" | "publish" => {
                out(std::process::Command::new("sh")
                    .arg(format!("{path}/{}", &u[0]))
                    .arg(root_path)
                    .arg(xpath)
                    .args(u[1..].iter())
                    .args(cmd.iter())
                    .current_dir(&path))?;
            }
            _ => {}
        }
    }
    std::thread::scope(|s| {
        if let Some(cargo) = member.cargo.as_ref() {
            s.spawn(|| {
                match (|| {
                    let mut val: toml::Table =
                        std::fs::read_to_string(format!("{path}/Cargo.toml"))?
                            .parse()
                            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
                    if update {
                        if let Some(p) = val.get_mut("package").and_then(|a| a.as_table_mut()) {
                            p.insert(
                                "version".to_owned(),
                                toml::Value::String(member.version.clone()),
                            );
                            p.insert(
                                "description".to_owned(),
                                toml::Value::String(member.description.clone()),
                            );
                            p.insert("publish".to_owned(), toml::Value::Boolean(!member.private));
                        }
                    }
                    match &*cmd[0] {
                        "build" | "publish" => {
                            out(std::process::Command::new("cargo")
                                .arg("check")
                                .current_dir(&path))?;
                        }
                        _ => {}
                    }
                    match &*cmd[0] {
                        "publish" if !member.private => {
                            out(std::process::Command::new("cargo")
                                .arg("publish")
                                .current_dir(&path))?;
                        }
                        "build" => {}
                        _ => {}
                    }
                    std::fs::write(
                        format!("{path}/Cargo.toml"),
                        toml::to_string_pretty(&val)
                            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?,
                    )?;
                    Ok::<_, std::io::Error>(())
                })() {
                    Ok(_) => {}
                    Err(e) => {
                        error.set(e);
                    }
                }
            });
        }
        if let Some(npm) = member.npm.as_ref() {
            s.spawn(|| {
                match (|| {
                    let mut val: serde_json::Value =
                        serde_json::from_reader(File::open(format!("{path}/package.json"))?)?;
                    if update {
                        for (a, b) in [
                            ("version", &member.version),
                            ("description", &member.description),
                        ] {
                            if let Some(o) = val.as_object_mut() {
                                o.insert(a.to_owned(), serde_json::Value::String(b.clone()));
                            }
                        }
                        if let Some(deps) = val
                            .as_object_mut()
                            .and_then(|o| o.get_mut("dependencies"))
                            .and_then(|d| d.as_object_mut())
                        {
                            for (k, v) in deps.iter_mut() {
                                if let Some(dep_name) = depmap
                                    .rnpm(root, root_path)?
                                    .get(k)
                                    .and_then(|a| root.members.get(a))
                                {
                                    *v = serde_json::Value::String(format!(
                                        "^{}",
                                        &dep_name.version
                                    ));
                                }
                            }
                        }
                    }
                    match &*cmd[0] {
                        "build" | "publish" => match val.get("zshy") {
                            Some(_) => {
                                std::fs::write(
                                    format!("{path}/package.json"),
                                    serde_json::to_vec_pretty(&val)?,
                                )?;
                                out(std::process::Command::new("npx")
                                    .arg("zshy")
                                    .arg("-p")
                                    .arg(format!("{root_path}/tsconfig.json"))
                                    .current_dir(&path))?;
                                val = serde_json::from_reader(File::open(format!(
                                    "{path}/package.json"
                                ))?)?;
                            }
                            None => match val.get("source") {
                                Some(_) => {
                                    out(std::process::Command::new("npx")
                                        .arg("parcel")
                                        .arg("build")
                                        .arg(format!("./{xpath}"))
                                        .current_dir(&root_path))?;
                                }
                                None => {}
                            },
                        },
                        _ => {}
                    }
                    match &*cmd[0] {
                        "publish" if !member.private => {
                            // build!();
                            out(std::process::Command::new("npm")
                                .arg("publish")
                                .arg("--access")
                                .arg("public")
                                .current_dir(&path))?;
                        }
                        "build" => {}
                        _ => {}
                    }
                    std::fs::write(
                        format!("{path}/package.json"),
                        serde_json::to_vec_pretty(&val)?,
                    )?;
                    Ok::<_, std::io::Error>(())
                })() {
                    Ok(_) => {}
                    Err(e) => {
                        error.set(e);
                    }
                }
            });
        }
    });
    if let Some(e) = error.take() {
        return Err(e);
    }
    Ok(())
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
pub struct Root {
    #[serde(rename = "//", default, skip_serializing_if = "Option::is_none")]
    pub core: Option<RootCore>,
    #[serde(flatten)]
    pub members: BTreeMap<String, Member>,
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
pub struct RootCore {}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
#[non_exhaustive]
pub struct Member {
    pub deps: BTreeMap<String, Dep>,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo: Option<Cargo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm: Option<NPM>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtree: Option<Subtree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updater: Option<Vec<String>>,
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
#[non_exhaustive]
pub struct Cargo {}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
#[non_exhaustive]
pub struct NPM {}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
#[non_exhaustive]
pub struct Subtree {
    pub paths: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
#[non_exhaustive]
pub struct Dep {
    pub subtree: Option<SubtreeID>,
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
pub struct SubtreeID {
    pub pkg_name: String,
    pub pkg: Box<Dep>,
    pub subtree: String,
    pub nest: Box<Dep>,
}
