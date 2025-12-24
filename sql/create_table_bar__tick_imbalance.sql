CREATE SCHEMA IF NOT EXISTS warehouse;

-- Table for Tick Imbalance Bars (TIBs)
CREATE TABLE IF NOT EXISTS warehouse.bar__tick_imbalance (
    time        TIMESTAMPTZ NOT NULL,
    symbol      TEXT NOT NULL,
    exchange    TEXT NOT NULL,
    interval    TEXT NOT NULL,
    open        DOUBLE PRECISION NOT NULL,
    high        DOUBLE PRECISION NOT NULL,
    low         DOUBLE PRECISION NOT NULL,
    close       DOUBLE PRECISION NOT NULL,
    volume      DOUBLE PRECISION NOT NULL,
    buy_ticks   BIGINT NOT NULL DEFAULT 0,
    sell_ticks  BIGINT NOT NULL DEFAULT 0,
    total_ticks BIGINT NOT NULL DEFAULT 0,
    theta       DOUBLE PRECISION NOT NULL DEFAULT 0.0
);

SELECT create_hypertable('warehouse.bar__tick_imbalance', 'time', if_not_exists => TRUE);
