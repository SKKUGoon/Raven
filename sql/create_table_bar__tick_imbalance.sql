CREATE SCHEMA IF NOT EXISTS mart;

-- Shared dimensions (star schema)
CREATE TABLE IF NOT EXISTS mart.dim_symbol (
    symbol_id   SERIAL PRIMARY KEY,
    symbol      TEXT NOT NULL UNIQUE,
    is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_date TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS mart.dim_exchange (
    exchange_id SERIAL PRIMARY KEY,
    exchange    TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS mart.dim_interval (
    interval_id SERIAL PRIMARY KEY,
    interval    TEXT NOT NULL UNIQUE
);

-- Fact table for Tick Imbalance / Tick Run Bars
CREATE TABLE IF NOT EXISTS mart.bar__tick_imbalance (
    time         TIMESTAMPTZ NOT NULL,
    symbol_id    INTEGER NOT NULL REFERENCES mart.dim_symbol(symbol_id),
    exchange_id  INTEGER NOT NULL REFERENCES mart.dim_exchange(exchange_id),
    interval_id  INTEGER NOT NULL REFERENCES mart.dim_interval(interval_id),
    open         DOUBLE PRECISION NOT NULL,
    high         DOUBLE PRECISION NOT NULL,
    low          DOUBLE PRECISION NOT NULL,
    close        DOUBLE PRECISION NOT NULL,
    volume       DOUBLE PRECISION NOT NULL,
    buy_ticks    BIGINT NOT NULL DEFAULT 0,
    sell_ticks   BIGINT NOT NULL DEFAULT 0,
    total_ticks  BIGINT NOT NULL DEFAULT 0,
    theta        DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    UNIQUE (time, symbol_id, exchange_id, interval_id)
);

SELECT create_hypertable('mart.bar__tick_imbalance', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS mart_tib_sid_eid_iid_time_idx
    ON mart.bar__tick_imbalance (symbol_id, exchange_id, interval_id, time DESC);
