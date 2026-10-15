use against_rome_saves::{Result, SaveService};
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        return Err("usage: against-rome-saves scan GAME BACKUPS | backup GAME BACKUPS FOLDER [TITLE LEVEL] | restore GAME BACKUPS ZIP FOLDER | repair-ai GAME BACKUPS FOLDER".into());
    }
    let svc = SaveService::new(&a[1], &a[2])?;
    match a[0].as_str() {
        "repair-ai" if a.len() == 4 => println!(
            "{}",
            serde_json::to_string_pretty(&svc.repair_endless_ai(&a[3])?)?
        ),
        "scan" if a.len() == 3 => println!("{}", serde_json::to_string_pretty(&svc.scan()?)?),
        "backup" if a.len() == 4 || a.len() == 6 => println!(
            "{}",
            svc.backup(
                &a[3],
                a.get(4).map_or("", String::as_str),
                a.get(5).map_or("", String::as_str)
            )?
        ),
        "restore" if a.len() == 5 => println!(
            "{}",
            serde_json::to_string_pretty(&svc.restore(&a[3], &a[4])?)?
        ),
        _ => return Err("invalid command or argument count".into()),
    }
    Ok(())
}
