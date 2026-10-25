pub fn privacy_policy() -> String {
    include_str!("../docs/privacy.md").into()
}

pub fn support() -> String {
    include_str!("../docs/cmds/support.md").into()
}

pub fn status() -> String {
    include_str!("../docs/status.md").into()
}

pub fn help() -> String {
    include_str!("../docs/help.md").into()
}

pub fn start() -> String {
    include_str!("../docs/cmds/start.txt").into()
}