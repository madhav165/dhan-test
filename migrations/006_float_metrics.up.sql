alter table backtest_runs
  alter column total_pnl type float8,
  alter column win_rate type float8,
  alter column max_drawdown type float8;
