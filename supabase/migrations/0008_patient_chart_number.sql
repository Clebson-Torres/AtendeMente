alter table public.patients
add column if not exists chart_number text;

create index if not exists patients_chart_number_idx
on public.patients (chart_number);

create unique index if not exists patients_user_chart_number_unique_idx
on public.patients (user_id, chart_number)
where deleted_at is null
  and chart_number is not null
  and btrim(chart_number) <> '';
