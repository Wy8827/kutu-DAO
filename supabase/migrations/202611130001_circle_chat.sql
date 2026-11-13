create extension if not exists pgcrypto with schema extensions;
create schema if not exists private;
revoke all on schema private from public, anon, authenticated;
grant usage on schema private to service_role;

create table public.chat_identities (
  auth_user_id uuid primary key references auth.users(id) on delete cascade,
  near_account_id text not null check (near_account_id ~ '^[a-z0-9._-]{2,64}$'),
  verified_at timestamptz not null default now()
);
alter table public.chat_identities enable row level security;
revoke all on public.chat_identities from public, anon, authenticated;
grant all on public.chat_identities to service_role;

create table public.circle_chat_memberships (
  auth_user_id uuid not null references auth.users(id) on delete cascade,
  near_account_id text not null check (near_account_id ~ '^[a-z0-9._-]{2,64}$'),
  circle_id text not null check (circle_id ~ '^[0-9]{1,20}$'),
  verified_at timestamptz not null default now(),
  primary key (auth_user_id, circle_id)
);
alter table public.circle_chat_memberships enable row level security;
revoke all on public.circle_chat_memberships from public, anon;
revoke insert, update, delete on public.circle_chat_memberships from authenticated;
grant select on public.circle_chat_memberships to authenticated;
grant all on public.circle_chat_memberships to service_role;

create table public.chat_profiles (
  near_account_id text primary key check (near_account_id ~ '^[a-z0-9._-]{2,64}$'),
  display_name text not null check (char_length(btrim(display_name)) between 1 and 48),
  avatar_url text check (avatar_url is null or (char_length(avatar_url) <= 2048 and avatar_url ~ '^https://')),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
alter table public.chat_profiles enable row level security;
grant select on public.chat_profiles to authenticated;
grant insert, update on public.chat_profiles to authenticated;
grant all on public.chat_profiles to service_role;

create or replace function private.touch_chat_profile_updated_at()
returns trigger language plpgsql set search_path = ''
as $$
begin
  new.updated_at := now();
  return new;
end
$$;
revoke all on function private.touch_chat_profile_updated_at() from public, anon, authenticated;
create trigger chat_profiles_updated_at
  before update on public.chat_profiles
  for each row execute function private.touch_chat_profile_updated_at();

create table public.chat_messages (
  id uuid primary key default gen_random_uuid(),
  circle_id text not null check (circle_id ~ '^[0-9]{1,20}$'),
  sender_near_account_id text not null references public.chat_profiles(near_account_id),
  content text not null check (char_length(content) between 1 and 1000 and char_length(btrim(content)) > 0),
  created_at timestamptz not null default now(),
  edited_at timestamptz
);
create index chat_messages_circle_created_idx on public.chat_messages (circle_id, created_at desc, id desc);
alter table public.chat_messages enable row level security;
grant select, insert, update on public.chat_messages to authenticated;
grant all on public.chat_messages to service_role;

create table private.near_chat_auth_challenges (
  id uuid primary key default gen_random_uuid(),
  auth_user_id uuid not null references auth.users(id) on delete cascade,
  near_account_id text not null check (near_account_id ~ '^[a-z0-9._-]{2,64}$'),
  nonce bytea not null check (octet_length(nonce) = 32),
  message text not null,
  recipient text not null,
  verification_attempts integer not null default 0 check (verification_attempts between 0 and 5),
  created_at timestamptz not null default now(),
  expires_at timestamptz not null,
  consumed_at timestamptz
);
alter table private.near_chat_auth_challenges enable row level security;
revoke all on private.near_chat_auth_challenges from public, anon, authenticated;
create index near_chat_challenges_user_created_idx on private.near_chat_auth_challenges (auth_user_id, created_at desc);

create table private.chat_message_rate_limits (
  auth_user_id uuid primary key references auth.users(id) on delete cascade,
  window_start timestamptz not null,
  message_count integer not null check (message_count >= 0)
);
alter table private.chat_message_rate_limits enable row level security;
revoke all on private.chat_message_rate_limits from public, anon, authenticated;

create or replace function public.get_my_near_account()
returns text language sql stable security definer set search_path = ''
as $$
  select i.near_account_id from public.chat_identities i
  where i.auth_user_id = (select auth.uid())
$$;
revoke all on function public.get_my_near_account() from public, anon;
grant execute on function public.get_my_near_account() to authenticated;

create or replace function public.is_circle_chat_member(p_circle_id text)
returns boolean language sql stable security definer set search_path = ''
as $$
  select exists (
    select 1 from public.circle_chat_memberships m
    where m.auth_user_id = (select auth.uid())
      and m.circle_id = p_circle_id
      and m.near_account_id = public.get_my_near_account()
      and m.verified_at > now() - interval '5 minutes'
  )
$$;
revoke all on function public.is_circle_chat_member(text) from public, anon;
grant execute on function public.is_circle_chat_member(text) to authenticated;

create or replace function public.get_my_near_chat_identity()
returns text language sql stable security definer set search_path = ''
as $$ select public.get_my_near_account() $$;
revoke all on function public.get_my_near_chat_identity() from public, anon;
grant execute on function public.get_my_near_chat_identity() to authenticated;

create or replace function public.can_read_chat_profile(p_near_account_id text)
returns boolean language sql stable security definer set search_path = ''
as $$
  select p_near_account_id = public.get_my_near_account() or exists (
    select 1
    from public.circle_chat_memberships mine
    join public.circle_chat_memberships peer on peer.circle_id = mine.circle_id
    where mine.auth_user_id = (select auth.uid())
      and mine.near_account_id = public.get_my_near_account()
      and mine.verified_at > now() - interval '5 minutes'
      and peer.near_account_id = p_near_account_id
      and peer.verified_at > now() - interval '5 minutes'
  )
$$;
revoke all on function public.can_read_chat_profile(text) from public, anon;
grant execute on function public.can_read_chat_profile(text) to authenticated;

create policy "wallet owner can read profiles"
  on public.chat_profiles for select to authenticated using (public.can_read_chat_profile(near_account_id));
create policy "wallet owner can insert profile"
  on public.chat_profiles for insert to authenticated
  with check (near_account_id = public.get_my_near_account());
create policy "wallet owner can update profile"
  on public.chat_profiles for update to authenticated
  using (near_account_id = public.get_my_near_account())
  with check (near_account_id = public.get_my_near_account());

create policy "members can read circle chat messages"
  on public.chat_messages for select to authenticated
  using (public.is_circle_chat_member(circle_id));
create policy "members can send circle chat messages"
  on public.chat_messages for insert to authenticated
  with check (public.is_circle_chat_member(circle_id)
    and sender_near_account_id = public.get_my_near_account());
create policy "authors can edit their messages"
  on public.chat_messages for update to authenticated
  using (public.is_circle_chat_member(circle_id)
    and sender_near_account_id = public.get_my_near_account())
  with check (public.is_circle_chat_member(circle_id)
    and sender_near_account_id = public.get_my_near_account());

create or replace function private.set_chat_message_sender()
returns trigger language plpgsql security definer set search_path = ''
as $$
declare v_account text;
begin
  v_account := public.get_my_near_account();
  if v_account is null then
    raise exception 'NEAR wallet authentication required' using errcode = '28000';
  end if;
  if tg_op = 'INSERT' then
    new.sender_near_account_id := v_account;
    return new;
  end if;
  if new.id is distinct from old.id
    or new.circle_id is distinct from old.circle_id
    or new.sender_near_account_id is distinct from old.sender_near_account_id
    or new.created_at is distinct from old.created_at then
    raise exception 'message identity fields are immutable' using errcode = '42501';
  end if;
  new.edited_at := case when new.content is distinct from old.content then now() else old.edited_at end;
  return new;
end
$$;
revoke all on function private.set_chat_message_sender() from public, anon, authenticated;
create trigger chat_messages_sender_and_immutability
  before insert or update on public.chat_messages
  for each row execute function private.set_chat_message_sender();

create or replace function private.enforce_chat_rate_limit()
returns trigger language plpgsql security definer set search_path = ''
as $$
declare
  v_user uuid := auth.uid();
  v_window timestamptz := date_trunc('minute', clock_timestamp());
  v_count integer;
begin
  if v_user is null then
    raise exception 'authenticated session required' using errcode = '28000';
  end if;
  insert into private.chat_message_rate_limits as current_limit (auth_user_id, window_start, message_count)
  values (v_user, v_window, 1)
  on conflict (auth_user_id) do update
    set window_start = case when current_limit.window_start < excluded.window_start then excluded.window_start else current_limit.window_start end,
        message_count = case when current_limit.window_start < excluded.window_start then 1 else current_limit.message_count + 1 end
  returning message_count into v_count;
  if v_count > 20 then
    raise exception 'chat message rate limit exceeded' using errcode = '54000';
  end if;
  return new;
end
$$;
revoke all on function private.enforce_chat_rate_limit() from public, anon, authenticated;
create trigger chat_messages_rate_limit
  before insert on public.chat_messages
  for each row execute function private.enforce_chat_rate_limit();

create or replace function public.issue_near_chat_challenge(p_auth_user_id uuid, p_near_account_id text, p_recipient text)
returns table (challenge_id uuid, nonce_base64 text, message text, expires_at timestamptz)
language plpgsql security definer set search_path = ''
as $$
declare
  v_id uuid := gen_random_uuid();
  v_nonce bytea := extensions.gen_random_bytes(32);
  v_expiry timestamptz := now() + interval '5 minutes';
  v_message text;
begin
  if p_near_account_id is null or p_near_account_id !~ '^[a-z0-9._-]{2,64}$' then
    raise exception 'invalid NEAR account ID' using errcode = '22023';
  end if;
  if p_recipient is null or char_length(p_recipient) not between 3 and 200 then
    raise exception 'invalid NEAR signature recipient' using errcode = '22023';
  end if;
  delete from private.near_chat_auth_challenges c where c.created_at < now() - interval '1 day';
  perform pg_advisory_xact_lock(hashtextextended(p_auth_user_id::text, 847291));
  if (select count(*) from private.near_chat_auth_challenges c
      where c.auth_user_id = p_auth_user_id and c.created_at > now() - interval '5 minutes') >= 10 then
    raise exception 'too many authentication challenges' using errcode = '54000';
  end if;
  v_message := 'Kutu DAO chat sign-in' || E'\n' || 'User: ' || p_auth_user_id::text || E'\n'
    || 'NEAR account: ' || p_near_account_id || E'\n'
    || 'Challenge: ' || v_id::text || E'\n' || 'Expires: ' || v_expiry::text;
  insert into private.near_chat_auth_challenges(id, auth_user_id, near_account_id, nonce, message, recipient, expires_at)
    values (v_id, p_auth_user_id, p_near_account_id, v_nonce, v_message, p_recipient, v_expiry);
  return query select v_id, encode(v_nonce, 'base64'), v_message, v_expiry;
end
$$;

create or replace function public.get_near_chat_challenge(p_challenge_id uuid, p_auth_user_id uuid)
returns table (nonce_base64 text, message text, recipient text, expires_at timestamptz)
language plpgsql security definer set search_path = ''
as $$
begin
  return query update private.near_chat_auth_challenges c
    set verification_attempts = c.verification_attempts + 1
    where c.id = p_challenge_id and c.auth_user_id = p_auth_user_id
      and c.consumed_at is null and c.expires_at > now() and c.verification_attempts < 5
    returning encode(c.nonce, 'base64'), c.message, c.recipient, c.expires_at;
end
$$;

create or replace function public.complete_near_chat_auth(
  p_challenge_id uuid, p_auth_user_id uuid, p_near_account_id text, p_recipient text
)
returns text language plpgsql security definer set search_path = ''
as $$
declare v_challenge private.near_chat_auth_challenges%rowtype;
begin
  if p_near_account_id !~ '^[a-z0-9._-]{2,64}$' then
    raise exception 'invalid NEAR account ID' using errcode = '22023';
  end if;
  select * into v_challenge from private.near_chat_auth_challenges c
    where c.id = p_challenge_id and c.auth_user_id = p_auth_user_id for update;
  if not found or v_challenge.consumed_at is not null or v_challenge.expires_at <= now()
    or v_challenge.recipient <> p_recipient or v_challenge.near_account_id <> p_near_account_id then
    raise exception 'challenge expired, mismatched, or already used' using errcode = '28000';
  end if;
  if exists (select 1 from public.chat_identities i
    where i.auth_user_id = p_auth_user_id and i.near_account_id <> p_near_account_id) then
    raise exception 'this Supabase session is already bound to another NEAR account; start a new session' using errcode = '28000';
  end if;
  update private.near_chat_auth_challenges set consumed_at = now() where id = p_challenge_id;
  insert into public.chat_identities(auth_user_id, near_account_id, verified_at)
    values (p_auth_user_id, p_near_account_id, now())
    on conflict (auth_user_id) do update set near_account_id = excluded.near_account_id, verified_at = excluded.verified_at;
  insert into public.chat_profiles(near_account_id, display_name)
    values (p_near_account_id, left(p_near_account_id, 48))
    on conflict (near_account_id) do nothing;
  return p_near_account_id;
end
$$;

revoke all on function public.issue_near_chat_challenge(uuid, text, text) from public, anon, authenticated;
revoke all on function public.get_near_chat_challenge(uuid, uuid) from public, anon, authenticated;
revoke all on function public.complete_near_chat_auth(uuid, uuid, text, text) from public, anon, authenticated;
grant execute on function public.issue_near_chat_challenge(uuid, text, text) to service_role;
grant execute on function public.get_near_chat_challenge(uuid, uuid) to service_role;
grant execute on function public.complete_near_chat_auth(uuid, uuid, text, text) to service_role;

do $$
begin
  if not exists (select 1 from pg_publication_tables
    where pubname = 'supabase_realtime' and schemaname = 'public' and tablename = 'chat_messages') then
    alter publication supabase_realtime add table public.chat_messages;
  end if;
end
$$;
