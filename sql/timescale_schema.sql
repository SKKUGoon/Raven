CREATE SCHEMA IF NOT EXISTS __SCHEMA__;

CREATE TABLE IF NOT EXISTS __SCHEMA__.dim__coin (
    coin_id      SERIAL PRIMARY KEY,
    coin         TEXT NOT NULL UNIQUE,
    is_deleted   BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_date TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS __SCHEMA__.dim__quote (
    quote_id     SERIAL PRIMARY KEY,
    quote        TEXT NOT NULL UNIQUE,
    is_deleted   BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_date TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS __SCHEMA__.dim__exchange (
    exchange_id  SERIAL PRIMARY KEY,
    exchange     TEXT NOT NULL UNIQUE,
    is_deleted   BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_date TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS __SCHEMA__.dim__interval (
    interval_id  SERIAL PRIMARY KEY,
    interval     TEXT NOT NULL UNIQUE,
    is_deleted   BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_date TIMESTAMPTZ NULL
);

ALTER TABLE __SCHEMA__.dim__coin ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE __SCHEMA__.dim__coin ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;
ALTER TABLE __SCHEMA__.dim__quote ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE __SCHEMA__.dim__quote ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;
ALTER TABLE __SCHEMA__.dim__exchange ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE __SCHEMA__.dim__exchange ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;
ALTER TABLE __SCHEMA__.dim__interval ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE __SCHEMA__.dim__interval ADD COLUMN IF NOT EXISTS deleted_date TIMESTAMPTZ NULL;

CREATE TABLE IF NOT EXISTS __SCHEMA__.fact__tick_imbalance (
    time        TIMESTAMPTZ NOT NULL,
    coin_id     INTEGER NOT NULL REFERENCES __SCHEMA__.dim__coin(coin_id),
    quote_id    INTEGER NOT NULL REFERENCES __SCHEMA__.dim__quote(quote_id),
    exchange_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__exchange(exchange_id),
    interval_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__interval(interval_id),
    open        DOUBLE PRECISION NOT NULL,
    high        DOUBLE PRECISION NOT NULL,
    low         DOUBLE PRECISION NOT NULL,
    close       DOUBLE PRECISION NOT NULL,
    volume      DOUBLE PRECISION NOT NULL,
    buy_ticks   BIGINT NOT NULL DEFAULT 0,
    sell_ticks  BIGINT NOT NULL DEFAULT 0,
    total_ticks BIGINT NOT NULL DEFAULT 0,
    theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
);

SELECT create_hypertable('__SCHEMA__.fact__tick_imbalance', 'time', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS __SCHEMA___fact_tib_cid_qid_eid_iid_time_idx
    ON __SCHEMA__.fact__tick_imbalance (coin_id, quote_id, exchange_id, interval_id, time DESC);

CREATE TABLE IF NOT EXISTS __SCHEMA__.fact__volume_imbalance (
    time        TIMESTAMPTZ NOT NULL,
    coin_id     INTEGER NOT NULL REFERENCES __SCHEMA__.dim__coin(coin_id),
    quote_id    INTEGER NOT NULL REFERENCES __SCHEMA__.dim__quote(quote_id),
    exchange_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__exchange(exchange_id),
    interval_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__interval(interval_id),
    open        DOUBLE PRECISION NOT NULL,
    high        DOUBLE PRECISION NOT NULL,
    low         DOUBLE PRECISION NOT NULL,
    close       DOUBLE PRECISION NOT NULL,
    volume      DOUBLE PRECISION NOT NULL,
    buy_ticks   BIGINT NOT NULL DEFAULT 0,
    sell_ticks  BIGINT NOT NULL DEFAULT 0,
    total_ticks BIGINT NOT NULL DEFAULT 0,
    theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
);

SELECT create_hypertable('__SCHEMA__.fact__volume_imbalance', 'time', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS __SCHEMA___fact_vib_cid_qid_eid_iid_time_idx
    ON __SCHEMA__.fact__volume_imbalance (coin_id, quote_id, exchange_id, interval_id, time DESC);

CREATE TABLE IF NOT EXISTS __SCHEMA__.fact__vpin (
    time        TIMESTAMPTZ NOT NULL,
    coin_id     INTEGER NOT NULL REFERENCES __SCHEMA__.dim__coin(coin_id),
    quote_id    INTEGER NOT NULL REFERENCES __SCHEMA__.dim__quote(quote_id),
    exchange_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__exchange(exchange_id),
    interval_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__interval(interval_id),
    open        DOUBLE PRECISION NOT NULL,
    high        DOUBLE PRECISION NOT NULL,
    low         DOUBLE PRECISION NOT NULL,
    close       DOUBLE PRECISION NOT NULL,
    volume      DOUBLE PRECISION NOT NULL,
    buy_ticks   BIGINT NOT NULL DEFAULT 0,
    sell_ticks  BIGINT NOT NULL DEFAULT 0,
    total_ticks BIGINT NOT NULL DEFAULT 0,
    theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
);

SELECT create_hypertable('__SCHEMA__.fact__vpin', 'time', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS __SCHEMA___fact_vpin_cid_qid_eid_iid_time_idx
    ON __SCHEMA__.fact__vpin (coin_id, quote_id, exchange_id, interval_id, time DESC);

CREATE TABLE IF NOT EXISTS __SCHEMA__.fact__kline (
    time        TIMESTAMPTZ NOT NULL,
    coin_id     INTEGER NOT NULL REFERENCES __SCHEMA__.dim__coin(coin_id),
    quote_id    INTEGER NOT NULL REFERENCES __SCHEMA__.dim__quote(quote_id),
    exchange_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__exchange(exchange_id),
    interval_id INTEGER NOT NULL REFERENCES __SCHEMA__.dim__interval(interval_id),
    open        DOUBLE PRECISION NOT NULL,
    high        DOUBLE PRECISION NOT NULL,
    low         DOUBLE PRECISION NOT NULL,
    close       DOUBLE PRECISION NOT NULL,
    volume      DOUBLE PRECISION NOT NULL,
    UNIQUE (time, coin_id, quote_id, exchange_id, interval_id)
);

SELECT create_hypertable('__SCHEMA__.fact__kline', 'time', if_not_exists => TRUE);
