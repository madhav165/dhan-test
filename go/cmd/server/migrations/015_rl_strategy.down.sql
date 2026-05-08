drop table if exists rl_jobs;
alter table strategies drop column if exists rl_summary;
alter table strategies drop column if exists rl_config;
alter table strategies drop column if exists strategy_type;
