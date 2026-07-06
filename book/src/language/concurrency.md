# Concurrency Surface

The parser accepts:

```spider
do together
    let weather = fetch_weather("Tokyo")
    let news = fetch_headlines()

spawn worker(jobs)
```

Current behavior is deliberately limited. M3 records that concurrency constructs
execute sequentially until M7, and the checker warns honestly with W0003.

Do not write production Spider code that relies on parallel execution yet.
