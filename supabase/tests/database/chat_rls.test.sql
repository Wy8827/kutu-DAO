begin;
select plan(34);

insert into auth.users (id, aud, role, email, encrypted_password, email_confirmed_at, raw_app_meta_data, raw_user_meta_data, created_at, updated_at, is_anonymous) values
  ('10000000-0000-0000-0000-000000000001','authenticated','authenticated','alice-chat-test@example.invalid','',now(),'{}','{}',now(),now(),true),
  ('10000000-0000-0000-0000-000000000002','authenticated','authenticated','bob-chat-test@example.invalid','',now(),'{}','{}',now(),now(),true);
insert into public.chat_identities (auth_user_id, near_account_id) values
  ('10000000-0000-0000-0000-000000000001','alice.testnet'),
  ('10000000-0000-0000-0000-000000000002','bob.testnet');
insert into public.circle_chat_memberships (auth_user_id, near_account_id, circle_id) values
  ('10000000-0000-0000-0000-000000000001','alice.testnet','10'),
  ('10000000-0000-0000-0000-000000000002','bob.testnet','20'),
  ('10000000-0000-0000-0000-000000000002','bob.testnet','10');
insert into public.chat_profiles (near_account_id, display_name) values ('alice.testnet','Alice'),('bob.testnet','Bob'),('charlie.testnet','Charlie') on conflict (near_account_id) do nothing;

create temporary table test_chat_challenges (slot integer primary key, challenge_id uuid);
insert into test_chat_challenges select 1, challenge_id from public.issue_near_chat_challenge('10000000-0000-0000-0000-000000000001','alice.testnet','kutu-dao-chat');
select throws_ok($$select public.complete_near_chat_auth((select challenge_id from test_chat_challenges where slot=1),'10000000-0000-0000-0000-000000000001','mallory.testnet','kutu-dao-chat')$$,'28000',null,'challenge cannot be completed for a different NEAR account');
select is((select message like '%NEAR account: alice.testnet%' from public.get_near_chat_challenge((select challenge_id from test_chat_challenges where slot=1),'10000000-0000-0000-0000-000000000001')),true,'issued signature message binds the NEAR account ID');
select lives_ok($$select public.complete_near_chat_auth((select challenge_id from test_chat_challenges where slot=1),'10000000-0000-0000-0000-000000000001','alice.testnet','kutu-dao-chat')$$,'challenge completes for the account embedded in its signed message');
select throws_ok($$select public.complete_near_chat_auth((select challenge_id from test_chat_challenges where slot=1),'10000000-0000-0000-0000-000000000001','alice.testnet','kutu-dao-chat')$$,'28000',null,'consumed challenge cannot be replayed');
insert into test_chat_challenges select 2, challenge_id from public.issue_near_chat_challenge('10000000-0000-0000-0000-000000000002','bob.testnet','kutu-dao-chat');
update private.near_chat_auth_challenges set expires_at = now() - interval '1 second' where id = (select challenge_id from test_chat_challenges where slot=2);
select throws_ok($$select public.complete_near_chat_auth((select challenge_id from test_chat_challenges where slot=2),'10000000-0000-0000-0000-000000000002','bob.testnet','kutu-dao-chat')$$,'28000',null,'expired challenge cannot be completed');
insert into test_chat_challenges select 3, challenge_id from public.issue_near_chat_challenge('10000000-0000-0000-0000-000000000002','bob.testnet','kutu-dao-chat');
select lives_ok($$do $test$ begin for i in 1..5 loop perform * from public.get_near_chat_challenge((select challenge_id from test_chat_challenges where slot=3),'10000000-0000-0000-0000-000000000002'); end loop; end $test$;$$,'five challenge verification attempts are allowed');
select is((select count(*)::integer from public.get_near_chat_challenge((select challenge_id from test_chat_challenges where slot=3),'10000000-0000-0000-0000-000000000002')),0,'challenge verification attempt limit blocks a sixth attempt');

set local role authenticated;
select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000002',true);
select is(public.get_my_near_chat_identity(),'bob.testnet','authenticated identity lookup returns the verified wallet for this session');
select throws_ok($$select * from public.chat_identities$$,'42501',null,'clients cannot query server-managed wallet identity mappings directly');
select is((select count(*)::integer from public.circle_chat_memberships),0,'RLS hides server-managed membership cache rows from direct reads');
select throws_ok($$insert into public.circle_chat_memberships (auth_user_id,near_account_id,circle_id) values ('10000000-0000-0000-0000-000000000002','bob.testnet','99')$$,'42501',null,'clients cannot insert membership cache records');
select throws_ok($$update public.circle_chat_memberships set circle_id='99' where auth_user_id='10000000-0000-0000-0000-000000000002' and circle_id='20'$$,'42501',null,'clients cannot alter membership cache records');
select throws_ok($$insert into public.chat_identities (auth_user_id,near_account_id) values ('10000000-0000-0000-0000-000000000002','mallory.testnet')$$,'42501',null,'clients cannot insert wallet identity mappings');
select throws_ok($$update public.chat_identities set near_account_id='mallory.testnet' where auth_user_id='10000000-0000-0000-0000-000000000002'$$,'42501',null,'clients cannot change wallet identity mappings');
select is((select count(*)::integer from public.chat_profiles where near_account_id='alice.testnet'),1,'circle member can read a current peer profile');
select is((select count(*)::integer from public.chat_profiles where near_account_id='charlie.testnet'),0,'circle member cannot enumerate profiles outside shared circles');
select lives_ok($$insert into public.chat_messages (id,circle_id,sender_near_account_id,content) values ('20000000-0000-0000-0000-000000000001','20','alice.testnet','bob message')$$,'member can create a message; trigger replaces forged sender identity');
select is((select sender_near_account_id from public.chat_messages where id='20000000-0000-0000-0000-000000000001'),'bob.testnet','message sender comes from verified wallet identity');

select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000001',true);
select lives_ok($$insert into public.chat_messages (id,circle_id,sender_near_account_id,content) values ('20000000-0000-0000-0000-000000000002','10','bob.testnet','alice message')$$,'member can create a circle message');
select is((select count(*)::integer from public.chat_messages where circle_id='10'),1,'member can retrieve own circle history');
select is((select count(*)::integer from public.chat_messages where circle_id='20'),0,'RLS hides another circle history');
reset role;
update public.circle_chat_memberships set verified_at = now() - interval '6 minutes' where auth_user_id = '10000000-0000-0000-0000-000000000002' and circle_id = '20';
set local role authenticated;
select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000002',true);
select is((select count(*)::integer from public.chat_messages where circle_id='20'),0,'expired membership cache cannot read circle history');
select throws_ok($$insert into public.chat_messages (circle_id,content) values ('20','expired membership cannot send')$$,'42501',null,'expired membership cache cannot send messages');
reset role;
update public.circle_chat_memberships set verified_at = now() where auth_user_id = '10000000-0000-0000-0000-000000000002' and circle_id = '20';
set local role authenticated;
select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000001',true);
select throws_ok($$insert into public.chat_messages (circle_id,content) values ('20','outsider message')$$,'42501',null,'RLS rejects sending to a circle without verified membership');
select lives_ok($$update public.chat_messages set content='edited by Alice' where id='20000000-0000-0000-0000-000000000002'$$,'author can edit own message');
select is((select edited_at is not null from public.chat_messages where id='20000000-0000-0000-0000-000000000002'),true,'editing records edited timestamp');
select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000002',true);
select lives_ok($$update public.chat_messages set content='Bob cannot edit Alice message' where id='20000000-0000-0000-0000-000000000002'$$,'RLS filters another member’s edit attempt');
select is((select content from public.chat_messages where id='20000000-0000-0000-0000-000000000002'),'edited by Alice','another member cannot change message content');
select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000001',true);
select throws_ok($$update public.chat_messages set sender_near_account_id='bob.testnet' where id='20000000-0000-0000-0000-000000000002'$$,'42501',null,'message sender identity is immutable');
select throws_ok($$insert into public.chat_messages (circle_id,content) values ('10','   ')$$,'23514',null,'empty messages are rejected');
select throws_ok($$insert into public.chat_messages (circle_id,content) values ('10',repeat('x',1001))$$,'23514',null,'messages over 1000 characters are rejected');
select lives_ok($$insert into public.chat_messages (circle_id,content) values ('10',repeat('x',1000))$$,'1000-character message is accepted');
select set_config('request.jwt.claim.sub','10000000-0000-0000-0000-000000000002',true);
select lives_ok($$do $test$ begin for i in 1..19 loop insert into public.chat_messages (circle_id,content) values ('20',format('rate limit test %s',i)); end loop; end $test$;$$,'session can send through its twentieth message in one minute');
select throws_ok($$insert into public.chat_messages (circle_id,content) values ('20','message over session limit')$$,'54000',null,'twenty-first message in one minute is rejected');

select * from finish();
rollback;
