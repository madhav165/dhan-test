create table if not exists rl_training_metrics (
  id          uuid primary key default gen_random_uuid(),
  strategy_id uuid not null references strategies(id) on delete cascade,
  episode     int not null,
  train_reward float8 not null,
  val_metric   float8,
  created_at  timestamptz default now()
);
create index if not exists idx_rl_metrics_strategy_episode on rl_training_metrics (strategy_id, episode);
