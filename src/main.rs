use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    // cell::OnceCell,
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{ErrorKind, Write, stderr, stdout},
    path::Path,
    process::Command,
    sync::{Mutex, RwLock},
};

/// Load a configuration from either JSON or YAML file.
/// Checks for JSON first, then YAML. This does NOT apply to package.json.
///
/// # Panics
/// Panics if `config_name` is "package" as package.json files should not use YAML.
pub fn load_config<T: for<'de> Deserialize<'de>>(
    base_path: &str,
    config_name: &str,
) -> std::io::Result<T> {
    // Enforce that package.json is not affected by YAML support
    assert!(
        config_name != "package",
        "package.json files are not supported by load_config. Use serde_json directly."
    );

    let json_path = format!("{base_path}/{config_name}.json");
    let yaml_path = format!("{base_path}/{config_name}.yaml");
    let yml_path = format!("{base_path}/{config_name}.yml");

    if Path::new(&json_path).exists() {
        let file = File::open(&json_path)?;
        serde_json::from_reader(file).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
    } else if Path::new(&yaml_path).exists() {
        let content = std::fs::read_to_string(&yaml_path)?;
        serde_yml::from_str(&content).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
    } else if Path::new(&yml_path).exists() {
        let content = std::fs::read_to_string(&yml_path)?;
        serde_yml::from_str(&content).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
    } else {
        Err(std::io::Error::new(
            ErrorKind::NotFound,
            format!(
                "Configuration file not found: {config_name}.json, {config_name}.yaml, or {config_name}.yml in {base_path}"
            ),
        ))
    }
}
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
        "schema" => {
            // Generate JSON schema for Root configuration
            let schema = schemars::generate::SchemaSettings::default()
                .into_generator()
                .into_root_schema_for::<Root>();
            let schema_json = serde_json::to_string_pretty(&schema)
                .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
            println!("{}", schema_json);
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
            let root: Root = load_config(&root_path, "pupi")?;
            let visited = RwLock::new(BTreeSet::new());
            let mut error = OnceCell::new();
            let d = DepMap::default();
            add_workspaces(&root, &root_path)?;
            std::thread::scope(|s| {
                for (path, member) in root.members.iter() {
                    // let path = format!("{root_path}/{path}");
                    s.spawn(|| {
                        match update(UpdateContext {
                            xpath: path,
                            root_path: &root_path,
                            member,
                            root: &root,
                            visited: &visited,
                            depmap: &d,
                            cmd: &[cmd.clone()]
                                .into_iter()
                                .chain(args.clone())
                                .collect::<Vec<_>>(),
                        }) {
                            Ok(_) => {}
                            Err(e) => {
                                let _ = error.set(e);
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
struct UpdateContext<'a> {
    xpath: &'a str,
    root_path: &'a str,
    member: &'a Member,
    root: &'a Root,
    visited: &'a RwLock<BTreeSet<String>>,
    depmap: &'a DepMap,
    cmd: &'a [String],
}

struct BuildContext<'a> {
    path: &'a str,
    root_path: &'a str,
    xpath: &'a str,
    member: &'a Member,
    root: &'a Root,
    depmap: &'a DepMap,
    cmd: &'a [String],
    update: bool,
}

#[derive(Default)]
struct DepMap {
    npm: OnceCell<BTreeMap<String, String>>,
    rnpm: OnceCell<BTreeMap<String, String>>,
    subroots: OnceCell<BTreeMap<String, SubrootEntry>>,
}
#[derive(Default)]
struct SubrootEntry {
    root: OnceCell<Root>,
    members: RwLock<BTreeSet<String>>,
    depmap: DepMap,
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
        let Some(SubrootEntry {
            root: m,
            members: r,
            depmap: n,
        }) = m.get(name)
        else {
            return Ok(None);
        };
        let m = m.get_or_try_init(|| {
            let subroot_path = format!("{root_path}/{name}");
            let root: Root = load_config(&subroot_path, "pupi")?;
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
fn update_dep(ctx: UpdateContext, dep: &Dep) -> std::io::Result<()> {
    let mut root_path = Cow::Borrowed(ctx.root_path);
    let mut root = ctx.root;
    let mut visited = ctx.visited;
    let mut depmap = ctx.depmap;
    let mut dep = dep;
    let do_update = matches!(&*ctx.cmd[0], "autogen" | "build" | "publish" | "update");
    loop {
        if let Some(s) = dep.subrepo.as_ref() {
            if do_update {
                return update(UpdateContext {
                    xpath: ctx.xpath,
                    root_path: &root_path,
                    member: ctx.member,
                    root,
                    visited,
                    depmap,
                    cmd: ctx.cmd,
                });
            }
            update_dep(
                UpdateContext {
                    xpath: &s.pkg_name,
                    root_path: &root_path,
                    member: ctx.member,
                    root,
                    visited,
                    depmap,
                    cmd: ctx.cmd,
                },
                &s.pkg,
            )?;
            if let Some((a, b, c, d)) =
                depmap.subroot(root, &root_path, &format!("{}/{}", &s.pkg_name, &s.subrepo))?
            {
                root_path = Cow::Owned(c);
                root = a;
                visited = b;
                depmap = d;
                dep = &s.nest;
                continue;
            }
        }
        return update(UpdateContext {
            xpath: ctx.xpath,
            root_path: &root_path,
            member: ctx.member,
            root,
            visited,
            depmap,
            cmd: ctx.cmd,
        });
    }
}
fn update(ctx: UpdateContext) -> std::io::Result<()> {
    if ctx.visited.read().unwrap().contains(ctx.xpath) {
        return Ok(());
    }
    match ctx.visited.write().unwrap() {
        mut w => {
            // let mut w = ;
            if w.contains(ctx.xpath) {
                return Ok(());
            }
            w.insert(ctx.xpath.to_owned());
        }
    };
    let path = format!("{}/{}", ctx.root_path, ctx.xpath);
    let update = matches!(&*ctx.cmd[0], "autogen" | "build" | "publish" | "update");
    let mut error = OnceCell::new();
    std::thread::scope(|s| {
        if let Some(subtree) = ctx.member.subtree.as_ref() {
            s.spawn(|| {
                match subtree.process(BuildContext {
                    path: &path,
                    root_path: ctx.root_path,
                    xpath: ctx.xpath,
                    member: ctx.member,
                    root: ctx.root,
                    depmap: ctx.depmap,
                    cmd: ctx.cmd,
                    update,
                }) {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = error.set(e);
                    }
                }
            });
        }
        if let Some(submodule) = ctx.member.submodule.as_ref() {
            s.spawn(|| {
                match submodule.process(BuildContext {
                    path: &path,
                    root_path: ctx.root_path,
                    xpath: ctx.xpath,
                    member: ctx.member,
                    root: ctx.root,
                    depmap: ctx.depmap,
                    cmd: ctx.cmd,
                    update,
                }) {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = error.set(e);
                    }
                }
            });
        }
    });
    if let Some(e) = error.take() {
        return Err(e);
    }
    std::thread::scope(|s| {
        for (dep, x) in ctx.member.deps.iter() {
            let error = &error;
            s.spawn(move || {
                match update_dep(
                    UpdateContext {
                        xpath: &dep,
                        root_path: ctx.root_path,
                        member: ctx.root.members.get(dep).unwrap(),
                        root: ctx.root,
                        visited: ctx.visited,
                        depmap: ctx.depmap,
                        cmd: ctx.cmd,
                    },
                    &x,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = error.set(e);
                    }
                }
            });
        }
    });
    if let Some(e) = error.take() {
        return Err(e);
    }
    eprintln!("[Build] Building {}", ctx.xpath);

    if let Some(u) = ctx.member.updater.as_ref() {
        match &*ctx.cmd[0] {
            "autogen" | "build" | "publish" => {
                out(std::process::Command::new("sh")
                    .arg(format!("{path}/{}", &u[0]))
                    .arg(ctx.root_path)
                    .arg(ctx.xpath)
                    .args(u[1..].iter())
                    .args(ctx.cmd.iter())
                    .current_dir(&path))?;
            }
            _ => {}
        }
    }
    std::thread::scope(|s| {
        if let Some(cargo) = ctx.member.cargo.as_ref() {
            s.spawn(|| {
                match cargo.process(BuildContext {
                    path: &path,
                    root_path: ctx.root_path,
                    xpath: ctx.xpath,
                    member: ctx.member,
                    root: ctx.root,
                    depmap: ctx.depmap,
                    cmd: ctx.cmd,
                    update,
                }) {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = error.set(e);
                    }
                }
            });
        }
        if let Some(npm) = ctx.member.npm.as_ref() {
            s.spawn(|| {
                match npm.process(BuildContext {
                    path: &path,
                    root_path: ctx.root_path,
                    xpath: ctx.xpath,
                    member: ctx.member,
                    root: ctx.root,
                    depmap: ctx.depmap,
                    cmd: ctx.cmd,
                    update,
                }) {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = error.set(e);
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
    pub submodule: Option<Submodule>,
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
pub struct Submodule {
    pub paths: BTreeMap<String, String>,
}

trait BuildSystem {
    fn process(&self, ctx: BuildContext) -> std::io::Result<()>;
}

impl BuildSystem for Cargo {
    fn process(&self, ctx: BuildContext) -> std::io::Result<()> {
        let mut val: toml::Table = std::fs::read_to_string(format!("{}/Cargo.toml", ctx.path))?
            .parse()
            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
        if ctx.update {
            if let Some(p) = val.get_mut("package").and_then(|a| a.as_table_mut()) {
                p.insert(
                    "version".to_owned(),
                    toml::Value::String(ctx.member.version.clone()),
                );
                p.insert(
                    "description".to_owned(),
                    toml::Value::String(ctx.member.description.clone()),
                );
                p.insert(
                    "publish".to_owned(),
                    toml::Value::Boolean(!ctx.member.private),
                );
            }
        }
        match &*ctx.cmd[0] {
            "build" | "publish" => {
                out(std::process::Command::new("cargo")
                    .arg("check")
                    .current_dir(&ctx.path))?;
            }
            _ => {}
        }
        match &*ctx.cmd[0] {
            "publish" if !ctx.member.private => {
                out(std::process::Command::new("cargo")
                    .arg("publish")
                    .current_dir(&ctx.path))?;
            }
            "build" => {}
            _ => {}
        }
        std::fs::write(
            format!("{}/Cargo.toml", ctx.path),
            toml::to_string_pretty(&val).map_err(|e| std::io::Error::new(ErrorKind::Other, e))?,
        )?;
        Ok(())
    }
}

impl BuildSystem for NPM {
    fn process(&self, ctx: BuildContext) -> std::io::Result<()> {
        let mut val: serde_json::Value =
            serde_json::from_reader(File::open(format!("{}/package.json", ctx.path))?)?;
        if ctx.update {
            for (a, b) in [
                ("version", &ctx.member.version),
                ("description", &ctx.member.description),
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
                    if let Some(dep_name) = ctx
                        .depmap
                        .rnpm(ctx.root, ctx.root_path)?
                        .get(k)
                        .and_then(|a| ctx.root.members.get(a))
                    {
                        *v = serde_json::Value::String(format!("^{}", &dep_name.version));
                    }
                }
            }
        }
        match &*ctx.cmd[0] {
            "build" | "publish" => match val.get("zshy") {
                Some(_) => {
                    std::fs::write(
                        format!("{}/package.json", ctx.path),
                        serde_json::to_vec_pretty(&val)?,
                    )?;
                    out(std::process::Command::new("npx")
                        .arg("zshy")
                        .arg("-p")
                        .arg(format!("{}/tsconfig.json", ctx.root_path))
                        .current_dir(&ctx.path))?;
                    val =
                        serde_json::from_reader(File::open(format!("{}/package.json", ctx.path))?)?;
                }
                None => match val.get("source") {
                    Some(_) => {
                        out(std::process::Command::new("npx")
                            .arg("parcel")
                            .arg("build")
                            .arg(format!("./{}", ctx.xpath))
                            .current_dir(&ctx.root_path))?;
                    }
                    None => {}
                },
            },
            _ => {}
        }
        match &*ctx.cmd[0] {
            "publish" if !ctx.member.private => {
                out(std::process::Command::new("npm")
                    .arg("publish")
                    .arg("--access")
                    .arg("public")
                    .current_dir(&ctx.path))?;
            }
            "build" => {}
            _ => {}
        }
        std::fs::write(
            format!("{}/package.json", ctx.path),
            serde_json::to_vec_pretty(&val)?,
        )?;
        Ok(())
    }
}

impl BuildSystem for Subtree {
    fn process(&self, ctx: BuildContext) -> std::io::Result<()> {
        let mut error = OnceCell::new();
        std::thread::scope(|s| {
            for (p, v) in self.paths.iter().map(|(p, v)| (p.clone(), v.clone())) {
                let error = &error;
                let root_path = ctx.root_path;
                let xpath = ctx.xpath;
                s.spawn(move || {
                    match (move || {
                        out(std::process::Command::new("git")
                            .arg("subtree")
                            .arg("pull")
                            .arg("-P")
                            .arg(format!("{root_path}/{xpath}/{p}"))
                            .arg(v))?;
                        Ok::<_, std::io::Error>(())
                    })() {
                        Ok(_) => {}
                        Err(e) => {
                            let _ = error.set(e);
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
}

impl BuildSystem for Submodule {
    fn process(&self, ctx: BuildContext) -> std::io::Result<()> {
        let mut error = OnceCell::new();
        std::thread::scope(|s| {
            for (p, v) in self.paths.iter().map(|(p, v)| (p.clone(), v.clone())) {
                let error = &error;
                let root_path = ctx.root_path;
                let xpath = ctx.xpath;
                s.spawn(move || {
                    match (move || {
                        let submodule_path = format!("{root_path}/{xpath}/{p}");
                        // Add submodule if it doesn't exist
                        if !std::fs::exists(&submodule_path)?
                            || std::fs::read_dir(&submodule_path)?.next().is_none()
                        {
                            out(std::process::Command::new("git")
                                .arg("submodule")
                                .arg("add")
                                .arg("-f")
                                .arg(&v)
                                .arg(&p)
                                .current_dir(&format!("{root_path}/{xpath}")))?;
                        }
                        // Update/pull the submodule
                        out(std::process::Command::new("git")
                            .arg("submodule")
                            .arg("update")
                            .arg("--init")
                            .arg("--recursive")
                            .arg("--remote")
                            .arg(&submodule_path))?;
                        Ok::<_, std::io::Error>(())
                    })() {
                        Ok(_) => {}
                        Err(e) => {
                            let _ = error.set(e);
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
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
#[non_exhaustive]
pub struct Dep {
    pub subrepo: Option<SubrepoID>,
}
#[derive(Serialize, Deserialize, JsonSchema, Default)]
pub struct SubrepoID {
    pub pkg_name: String,
    pub pkg: Box<Dep>,
    pub subrepo: String,
    pub nest: Box<Dep>,
}
