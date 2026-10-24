## Commands used in this library crate

- Command to add a new .sql migration file : sqlx migrate add init --source [source folder]/migrations
- Command to run Postgres sql in docker : docker compose up -d
- Command to Stop Postgres sql in docker: docker compose down
- Command to create sqlx database: sqlx database create
- Command to migrate sql file : sqlx migrate run --source [source folder]/migrations
- Within the docker desktop's exec for the newly created postgres instance, run this command to view the tables: psql -U [postgres username] -d [postgres_db name], it will show data like below :

  List of relations
  Schema | Name | Type | Owner  
  --------+-------------------+-------+----------------
  public | \_sqlx_migrations | table | vijay_postgres
  public | delivery_attempts | table | vijay_postgres
  public | outbox_events | table | vijay_postgres
  (3 rows)
