use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::ErrorKind,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    args.next();
    let cmd = args.next().unwrap();
    let root_path = args.next().unwrap();
    let args = args.collect::<Vec<_>>();
    let root: Root = serde_json::from_reader(File::open(format!("{root_path}/pupi.json"))?)?;
    let mut visited = BTreeSet::new();
    for (path, member) in root.members.iter() {
        // let path = format!("{root_path}/{path}");
        update(
            path,
            &root_path,
            member,
            &root,
            &mut visited,
            &[cmd.clone()]
                .into_iter()
                .chain(args.clone())
                .collect::<Vec<_>>(),
        )?;
    }
    Ok(())
}

fn update(
    xpath: &str,
    root_path: &str,
    member: &Member,
    root: &Root,
    visited: &mut BTreeSet<String>,
    cmd: &[String],
) -> std::io::Result<()> {
    if visited.contains(xpath) {
        return Ok(());
    }
    visited.insert(xpath.to_owned());
    for dep in member.deps.iter() {
        update(
            &dep,
            root_path,
            root.members.get(dep).unwrap(),
            root,
            visited,
            cmd,
        )?;
    }
    let path = format!("{root_path}/{xpath}");
    if let Some(cargo) = member.cargo.as_ref() {
        let mut val: toml::Table = std::fs::read_to_string(format!("{path}/Cargo.toml"))?
            .parse()
            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;

        match &*cmd[0] {
            "publish" => {
                std::process::Command::new("cargo")
                    .arg("publish")
                    .current_dir(&path)
                    .spawn()?
                    .wait()?;
            }
            "build" => {}
            _ => {}
        }

        std::fs::write(
            format!("{path}/Cargo.toml"),
            toml::to_string_pretty(&val).map_err(|e| std::io::Error::new(ErrorKind::Other, e))?,
        )?;
    }
    if let Some(npm) = member.npm.as_ref() {
        let mut val: serde_json::Value =
            serde_json::from_reader(File::open(format!("{path}/package.json"))?)?;

        match &*cmd[0] {
            "build" | "publish" => match val.get("zshy") {
                Some(_) => {
                    std::process::Command::new("npx")
                        .arg("zshy")
                        .current_dir(&path)
                        .spawn()?
                        .wait()?;
                    val = serde_json::from_reader(File::open(format!("{path}/package.json"))?)?;
                }
                None => match val.get("source") {
                    Some(_) => {
                        std::process::Command::new("npx")
                            .arg("parcel")
                            .arg("build")
                            .arg(format!("./{xpath}"))
                            .current_dir(&root_path)
                            .spawn()?
                            .wait()?;
                    }
                    None => {}
                },
            },
            _ => {}
        }
        match &*cmd[0] {
            "publish" => {
                // build!();
                std::process::Command::new("npm")
                    .arg("publish")
                    .arg("--access")
                    .arg("public")
                    .current_dir(&path)
                    .spawn()?
                    .wait()?;
            }
            "build" => {}
            _ => {}
        }
        std::fs::write(
            format!("{path}/package.json"),
            serde_json::to_vec_pretty(&val)?,
        )?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Root {
    #[serde(flatten)]
    pub members: BTreeMap<String, Member>,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Member {
    pub deps: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo: Option<Cargo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm: Option<NPM>,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Cargo {}
#[derive(Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct NPM {}
