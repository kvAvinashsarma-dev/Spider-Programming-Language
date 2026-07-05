fn load() -> Outcome of Settings
    let settings = try read_settings("app.cfg")
    return Ok(settings)

fn main()
    match read_settings("app.cfg")
        Ok(settings) -> apply(settings)
        Fail(problem) -> say "Could not load settings: {problem}"
    let settings = try read_settings("app.cfg") else default_settings()
    say settings
