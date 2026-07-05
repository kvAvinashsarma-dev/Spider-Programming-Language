do together
    let weather = fetch_weather("Tokyo")
    let news = fetch_headlines()

spawn worker(jobs)
say "started"
