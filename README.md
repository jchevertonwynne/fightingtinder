# fightingtinder

Backend for fightingtinder, implemented in rust using actix web, diesel and postgres. Frontend can be found at [fightingtinder-frontend](https://github.com/jchevertonwynne/fightingtinder-frontend)

## How to run

Copy `.env.fake` to `.env` and create your own secret values. Do not use the defaults, as they're not secret anymore.

`docker run -d -p 6379:6379 -v redis.conf:/usr/local/etc/redis --name fightingtinder-redis redis`

`docker run -d -p 6000:5432 --name fightingtinder-postgres -e POSTGRES_PASSWORD=yourPassword postgres -N 200`

`cargo run` to start application in debug mode, `cargo run --release` for full speed version. To build a binary, use `cargo build --release` and run using `./target/release/fightingtinder`