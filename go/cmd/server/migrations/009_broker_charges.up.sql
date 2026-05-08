create table broker_charges (
    trade_type       text    primary key, -- 'intraday', 'delivery'
    brokerage_flat   numeric not null,    -- max flat fee per order (INR)
    brokerage_pct    numeric not null,    -- % of order value per order
    stt_buy_pct      numeric not null,
    stt_sell_pct     numeric not null,
    exchange_pct     numeric not null,    -- NSE exchange transaction charge
    sebi_pct         numeric not null,
    stamp_buy_pct    numeric not null,
    gst_pct          numeric not null
);

insert into broker_charges values
    ('intraday',  20, 0.000300, 0.000000, 0.000250, 0.000030699, 0.000001, 0.000030, 0.18),
    ('delivery',   0, 0.000000, 0.001000, 0.001000, 0.000030699, 0.000001, 0.000150, 0.18);
