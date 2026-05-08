alter table backtest_runs
  alter column total_pnl type numeric,
  alter column win_rate type numeric,
  alter column max_drawdown type numeric;
