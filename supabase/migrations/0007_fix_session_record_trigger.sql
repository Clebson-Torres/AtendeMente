create or replace function public.validate_patient_and_appointment_ownership()
returns trigger
language plpgsql
as $$
declare
  patient_owner uuid;
  appointment_owner uuid;
  appointment_patient uuid;
begin
  if tg_table_name = 'appointments' then
    select user_id into patient_owner from public.patients where id = new.patient_id and deleted_at is null;

    if patient_owner is distinct from new.user_id then
      raise exception 'Patient ownership mismatch';
    end if;

    return new;
  end if;

  select user_id into patient_owner from public.patients where id = new.patient_id and deleted_at is null;
  select user_id, patient_id into appointment_owner, appointment_patient
  from public.appointments where id = new.appointment_id and deleted_at is null;

  if patient_owner is distinct from new.user_id then
    raise exception 'Patient ownership mismatch';
  end if;

  if appointment_owner is distinct from new.user_id then
    raise exception 'Appointment ownership mismatch';
  end if;

  if appointment_patient is distinct from new.patient_id then
    raise exception 'Appointment patient mismatch';
  end if;

  return new;
end;
$$;

create or replace function public.validate_record_file_ownership()
returns trigger
language plpgsql
as $$
declare
  patient_owner uuid;
  appointment_owner uuid;
  appointment_patient uuid;
  payment_owner uuid;
  payment_appointment uuid;
begin
  select user_id into patient_owner from public.patients where id = new.patient_id and deleted_at is null;
  select user_id, patient_id into appointment_owner, appointment_patient
  from public.appointments where id = new.appointment_id and deleted_at is null;

  if patient_owner is distinct from new.user_id then
    raise exception 'Patient ownership mismatch';
  end if;

  if appointment_owner is distinct from new.user_id then
    raise exception 'Appointment ownership mismatch';
  end if;

  if appointment_patient is distinct from new.patient_id then
    raise exception 'Appointment patient mismatch';
  end if;

  if new.payment_id is not null then
    select user_id, appointment_id into payment_owner, payment_appointment
    from public.payments where id = new.payment_id and deleted_at is null;

    if payment_owner is distinct from new.user_id then
      raise exception 'Payment ownership mismatch';
    end if;

    if payment_appointment is distinct from new.appointment_id then
      raise exception 'Payment appointment mismatch';
    end if;
  end if;

  return new;
end;
$$;

drop trigger if exists validate_record_file_ownership on public.record_files;
create trigger validate_record_file_ownership
before insert or update on public.record_files
for each row execute function public.validate_record_file_ownership();
