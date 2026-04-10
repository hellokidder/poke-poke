pub fn play_alert() {
    std::thread::spawn(|| {
        let _ = std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .output();
    });
}
