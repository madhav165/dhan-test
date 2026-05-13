create table if not exists broker_charges (
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
    ('delivery',   0, 0.000000, 0.001000, 0.001000, 0.000030699, 0.000001, 0.000150, 0.18)
on conflict (trade_type) do update set
    brokerage_flat = excluded.brokerage_flat,
    brokerage_pct = excluded.brokerage_pct,
    stt_buy_pct = excluded.stt_buy_pct,
    stt_sell_pct = excluded.stt_sell_pct,
    exchange_pct = excluded.exchange_pct,
    sebi_pct = excluded.sebi_pct,
    stamp_buy_pct = excluded.stamp_buy_pct,
    gst_pct = excluded.gst_pct;
