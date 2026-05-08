drop table if exists trade_positions;
drop table if exists trade_jobs;
alter table policy_instruments
  drop column if exists quantity,
  drop column if exists order_type,
  drop column if exists max_trade_value;
