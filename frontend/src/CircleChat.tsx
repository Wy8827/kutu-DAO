import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { createClient } from '@supabase/supabase-js';
import type { NearWalletBase } from '@hot-labs/near-connect';
import { subscribeToCircleMessages, validateChatMessage, type ChatMessage } from './chat-utils';

const NETWORK = 'testnet' as const;
const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL || '';
const SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY || '';
const supabase = SUPABASE_URL && SUPABASE_ANON_KEY
  ? createClient(SUPABASE_URL, SUPABASE_ANON_KEY, { auth: { persistSession: true, autoRefreshToken: true, detectSessionInUrl: true } })
  : null;

type Props = { circleId: string; account: string; wallet: NearWalletBase | null };
type Profile = { near_account_id: string; display_name: string; avatar_url: string | null };
type DisplayMessage = ChatMessage & { displayName: string; avatarUrl: string | null };

function nonceBytes(value: string) {
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function displayName(accountId: string) {
  return accountId.length > 28 ? `${accountId.slice(0, 12)}…${accountId.slice(-10)}` : accountId;
}

export function CircleChat({ circleId, account, wallet }: Props) {
  const [verifiedAccount, setVerifiedAccount] = useState('');
  const [phase, setPhase] = useState<'idle' | 'authenticating' | 'authorizing' | 'loading' | 'ready' | 'error'>('idle');
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [draft, setDraft] = useState('');
  const [profile, setProfile] = useState<Profile | null>(null);
  const [profileDraft, setProfileDraft] = useState('');
  const [error, setError] = useState('');
  const [connection, setConnection] = useState('Connecting');
  const [online, setOnline] = useState(navigator.onLine);
  const [sending, setSending] = useState(false);
  const [savingProfile, setSavingProfile] = useState(false);
  const [atBottom, setAtBottom] = useState(true);
  const [hasOlder, setHasOlder] = useState(true);
  const listRef = useRef<HTMLDivElement>(null);
  const tailRef = useRef<HTMLDivElement>(null);
  const firstLoad = useRef(true);
  const oldestMessage = useRef<string | null>(null);

  const readProfile = useCallback(async (accountId: string) => {
    if (!supabase) return null;
    const { data, error: profileError } = await supabase.from('chat_profiles')
      .select('near_account_id,display_name,avatar_url').eq('near_account_id', accountId).maybeSingle();
    if (profileError) throw profileError;
    return data as Profile | null;
  }, []);

  const reloadLatest = useCallback(async () => {
    if (!supabase) return;
    const { data, error: historyError } = await supabase.from('chat_messages')
      .select('id,circle_id,sender_near_account_id,content,created_at,edited_at,chat_profiles!chat_messages_sender_near_account_id_fkey(display_name,avatar_url)')
      .eq('circle_id', circleId).order('created_at', { ascending: false }).limit(50);
    if (historyError) { setError(historyError.message); return; }
    const latest = (data || []).map((row: any) => ({
      id: row.id,
      circle_id: row.circle_id,
      sender_near_account_id: row.sender_near_account_id,
      content: row.content,
      created_at: row.created_at,
      edited_at: row.edited_at,
      displayName: row.chat_profiles?.display_name || displayName(row.sender_near_account_id),
      avatarUrl: row.chat_profiles?.avatar_url || null,
    } satisfies DisplayMessage));
    setMessages((current) => [...new Map([...current, ...latest].map((message) => [message.id, message])).values()]
      .sort((left, right) => left.created_at.localeCompare(right.created_at) || left.id.localeCompare(right.id)));
  }, [circleId]);

  useEffect(() => {
    setVerifiedAccount('');
    setPhase('idle');
    setError('');
    setMessages([]);
    setProfile(null);
    firstLoad.current = true;
    oldestMessage.current = null;
    setHasOlder(true);
    if (!supabase || !account) return;
    let active = true;
    void (async () => {
      const { data: { session } } = await supabase.auth.getSession();
      if (!active || !session) return;
      const { data } = await supabase.rpc('get_my_near_chat_identity');
      if (active && data === account) setVerifiedAccount(account);
    })().catch((reason: unknown) => { if (active) setError(reason instanceof Error ? reason.message : 'Could not check chat identity.'); });
    return () => { active = false; };
  }, [account]);

  useEffect(() => {
    const update = () => setOnline(navigator.onLine);
    window.addEventListener('online', update);
    window.addEventListener('offline', update);
    return () => { window.removeEventListener('online', update); window.removeEventListener('offline', update); };
  }, []);

  useEffect(() => {
    if (!online) setConnection('Offline');
    else if (phase === 'ready' && connection === 'Offline') setConnection('Reconnecting');
  }, [online, phase, connection]);

  useEffect(() => {
    if (!supabase || !circleId || verifiedAccount !== account || !account) return;
    let active = true;
    setPhase('authorizing');
    setError('');
    void (async () => {
      try {
        const { error: authzError } = await supabase.functions.invoke('authorize-circle-chat', { body: { circleId } });
        if (authzError) throw authzError;
        if (!active) return;
        setPhase('loading');
        const [profileRow, history] = await Promise.all([
          readProfile(account),
          supabase.from('chat_messages')
            .select('id,circle_id,sender_near_account_id,content,created_at,edited_at,chat_profiles!chat_messages_sender_near_account_id_fkey(display_name,avatar_url)')
            .eq('circle_id', circleId).order('created_at', { ascending: false }).limit(50),
        ]);
        if (history.error) throw history.error;
        const rows = (history.data || []).map((row: any) => ({
          id: row.id,
          circle_id: row.circle_id,
          sender_near_account_id: row.sender_near_account_id,
          content: row.content,
          created_at: row.created_at,
          edited_at: row.edited_at,
          displayName: row.chat_profiles?.display_name || displayName(row.sender_near_account_id),
          avatarUrl: row.chat_profiles?.avatar_url || null,
        } satisfies DisplayMessage)).reverse();
        if (!active) return;
        setProfile(profileRow);
        setProfileDraft(profileRow?.display_name || displayName(account));
        setMessages(rows);
        oldestMessage.current = rows[0]?.created_at || null;
        setHasOlder(rows.length === 50);
        setPhase('ready');
        setConnection(navigator.onLine ? 'Connecting' : 'Offline');
        firstLoad.current = true;
      } catch (reason) {
        if (!active) return;
        setPhase('error');
        setError(reason instanceof Error ? reason.message : 'Could not load circle chat.');
      }
    })();
    const refreshAuthorization = window.setInterval(() => {
      void supabase.functions.invoke('authorize-circle-chat', { body: { circleId } })
        .then(({ error: refreshError }) => {
          if (refreshError && active) {
            setPhase('error');
            setError('Circle membership verification expired. Reopen chat to verify membership again.');
          }
        });
    }, 4 * 60 * 1000);
    return () => { active = false; window.clearInterval(refreshAuthorization); };
  }, [account, circleId, readProfile, verifiedAccount]);

  useEffect(() => {
    if (!supabase || phase !== 'ready') return;
    let active = true;
    const stop = subscribeToCircleMessages(supabase, circleId, async (incoming) => {
      const senderProfile = incoming.sender_near_account_id === account ? profile : await readProfile(incoming.sender_near_account_id);
      const item: DisplayMessage = {
        ...incoming,
        displayName: senderProfile?.display_name || displayName(incoming.sender_near_account_id),
        avatarUrl: senderProfile?.avatar_url || null,
      };
      if (active) setMessages((current) => current.some((message) => message.id === item.id) ? current : [...current, item]);
    }, (status) => {
      if (active) {
        setConnection(status === 'SUBSCRIBED' ? 'Live' : status === 'CHANNEL_ERROR' || status === 'TIMED_OUT' ? 'Reconnecting' : status);
        if (status === 'SUBSCRIBED') void reloadLatest();
      }
    });
    return () => { active = false; stop(); };
  }, [account, circleId, phase, profile, readProfile, reloadLatest]);

  useEffect(() => {
    if (phase === 'ready' && firstLoad.current) {
      tailRef.current?.scrollIntoView({ behavior: 'auto', block: 'end' });
      firstLoad.current = false;
    } else if (atBottom) {
      tailRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
    }
  }, [messages, phase, atBottom]);

  async function authenticate() {
    if (!supabase || !wallet || !account) return;
    setPhase('authenticating');
    setError('');
    try {
      const { data: { session }, error: sessionError } = await supabase.auth.getSession();
      if (sessionError) throw sessionError;
      const { data: currentIdentity } = session ? await supabase.rpc('get_my_near_chat_identity') : { data: null };
      if (session && currentIdentity && currentIdentity !== account) {
        const { error: signOutError } = await supabase.auth.signOut({ scope: 'local' });
        if (signOutError) throw signOutError;
      }
      const { data: { session: activeSession } } = await supabase.auth.getSession();
      if (!activeSession) {
        const { error: signInError } = await supabase.auth.signInAnonymously();
        if (signInError) throw signInError;
      }
      const { data: challenge, error: challengeError } = await supabase.functions.invoke('near-chat-challenge', { body: { accountId: account } });
      if (challengeError || !challenge) throw challengeError || new Error('Could not create wallet challenge.');
      const signed = await wallet.signMessage({
        message: challenge.message,
        recipient: challenge.recipient,
        nonce: nonceBytes(challenge.nonce),
        network: NETWORK,
        signerId: account,
      });
      if (signed.accountId !== account) throw new Error('The wallet signed with a different account.');
      const { error: verifyError } = await supabase.functions.invoke('near-chat-verify', {
        body: { challengeId: challenge.challengeId, accountId: signed.accountId, publicKey: signed.publicKey, signature: signed.signature },
      });
      if (verifyError) throw verifyError;
      setVerifiedAccount(account);
      setPhase('authorizing');
    } catch (reason) {
      setPhase('error');
      setError(reason instanceof Error ? reason.message : 'Wallet authentication failed.');
    }
  }

  async function sendMessage(event: FormEvent) {
    event.preventDefault();
    if (!supabase || sending || phase !== 'ready') return;
    setError('');
    let content: string;
    try { content = validateChatMessage(draft); } catch (reason) { setError(reason instanceof Error ? reason.message : 'Invalid message.'); return; }
    setSending(true);
    try {
      const { data, error: insertError } = await supabase.from('chat_messages').insert({ circle_id: circleId, content })
        .select('id,circle_id,sender_near_account_id,content,created_at,edited_at,chat_profiles!chat_messages_sender_near_account_id_fkey(display_name,avatar_url)').single();
      if (insertError) throw insertError;
      setMessages((current) => current.some((message) => message.id === data.id) ? current : [...current, {
        id: data.id,
        circle_id: data.circle_id,
        sender_near_account_id: data.sender_near_account_id,
        content: data.content,
        created_at: data.created_at,
        edited_at: data.edited_at,
        displayName: (data.chat_profiles as any)?.display_name || displayName(data.sender_near_account_id),
        avatarUrl: (data.chat_profiles as any)?.avatar_url || null,
      }]);
      setDraft('');
      setAtBottom(true);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Could not send message.');
    } finally { setSending(false); }
  }

  async function loadOlder() {
    if (!supabase || !oldestMessage.current || !hasOlder) return;
    const before = oldestMessage.current;
    const { data, error: historyError } = await supabase.from('chat_messages')
      .select('id,circle_id,sender_near_account_id,content,created_at,edited_at,chat_profiles!chat_messages_sender_near_account_id_fkey(display_name,avatar_url)')
      .eq('circle_id', circleId).lt('created_at', before).order('created_at', { ascending: false }).limit(50);
    if (historyError) { setError(historyError.message); return; }
    const rows = (data || []).map((row: any) => ({
      ...row,
      displayName: row.chat_profiles?.display_name || displayName(row.sender_near_account_id),
      avatarUrl: row.chat_profiles?.avatar_url || null,
    } satisfies DisplayMessage)).reverse();
    oldestMessage.current = rows[0]?.created_at || before;
    setHasOlder(rows.length === 50);
    setMessages((current) => [...rows, ...current]);
  }

  async function saveProfile(event: FormEvent) {
    event.preventDefault();
    if (!supabase || !profile || savingProfile) return;
    const name = profileDraft.trim();
    if (!name || name.length > 48) { setError('Display names must contain 1–48 characters.'); return; }
    setSavingProfile(true);
    try {
      const { data, error: saveError } = await supabase.from('chat_profiles')
        .update({ display_name: name }).eq('near_account_id', account).select('near_account_id,display_name,avatar_url').single();
      if (saveError) throw saveError;
      setProfile(data as Profile);
      setMessages((current) => current.map((message) => message.sender_near_account_id === account ? { ...message, displayName: name } : message));
    } catch (reason) { setError(reason instanceof Error ? reason.message : 'Could not save profile.'); }
    finally { setSavingProfile(false); }
  }

  const configured = Boolean(supabase);
  return <section className="chat-panel" aria-label={`Circle ${circleId} chat`}>
    <div className="chat-heading"><div><span className="panel-kicker">CIRCLE #{circleId}</span><h4>Community chat</h4><p>Member conversation · separate from on-chain circle activity</p></div><span className={`chat-connection ${online ? 'online' : 'offline'}`}><i/>{online ? connection : 'Offline'}</span></div>
    {!configured ? <div className="chat-placeholder">Add the Supabase project URL and public anon key to <code>frontend/.env.local</code> to enable circle chat.</div>
      : !wallet || !account ? <div className="chat-placeholder">Connect a NEAR Testnet wallet to authenticate and open this circle’s chat.</div>
      : verifiedAccount !== account ? <div className="chat-auth"><p>Prove this wallet controls <b>{account}</b> before accessing circle chat.</p><button className="button button-dark button-small" disabled={phase === 'authenticating'} onClick={()=>void authenticate()}>{phase === 'authenticating'?'Waiting for wallet…':'Verify wallet for chat'}</button></div>
      : <>
        {profile&&<form className="chat-profile" onSubmit={(event)=>void saveProfile(event)}><label>Display name<input value={profileDraft} maxLength={48} onChange={(event)=>setProfileDraft(event.target.value)} aria-label="Chat display name"/></label><button disabled={savingProfile||profileDraft.trim()===profile.display_name}>{savingProfile?'Saving…':'Save name'}</button></form>}
        {phase === 'authorizing' || phase === 'loading' ? <div className="chat-state">{phase === 'authorizing'?'Checking membership on NEAR Testnet…':'Loading messages…'}</div>
          : phase === 'error' ? <div className="chat-state chat-error">{error}<button onClick={()=>{setVerifiedAccount('');setPhase('idle');}}>Try wallet verification again</button></div>
          : phase === 'ready' ? <>
            <div className="chat-history" ref={listRef} onScroll={(event)=>{const element=event.currentTarget;setAtBottom(element.scrollHeight-element.scrollTop-element.clientHeight<100);}}>
              {hasOlder&&messages.length>=50&&<button className="chat-load-older" onClick={()=>void loadOlder()}>Load earlier messages</button>}
              {messages.length===0 ? <div className="chat-empty">No messages yet. Start the conversation.</div> : messages.map((message)=><article className={`chat-message ${message.sender_near_account_id===account?'chat-own':''}`} key={message.id}>
                <div className="chat-avatar">{message.avatarUrl?<img src={message.avatarUrl} alt=""/>:message.displayName.slice(0,1).toUpperCase()}</div>
                <div className="chat-message-body"><div className="chat-message-meta"><b>{message.displayName}</b><time dateTime={message.created_at}>{new Date(message.created_at).toLocaleString()}</time>{message.edited_at&&<small>edited</small>}</div><p>{message.content}</p></div>
              </article>)}
              <div ref={tailRef}/>
            </div>
            <form className="chat-compose" onSubmit={(event)=>void sendMessage(event)}><textarea value={draft} maxLength={1000} rows={2} placeholder="Write a message…" aria-label="Chat message" onChange={(event)=>setDraft(event.target.value)} onKeyDown={(event)=>{if(event.key==='Enter'&&!event.shiftKey){event.preventDefault();void sendMessage(event);}}}/><div><small>{draft.trim().length}/1000 · Enter to send, Shift+Enter for a new line</small><button className="button button-dark button-small" disabled={sending||!draft.trim()}>{sending?'Sending…':'Send message'}</button></div></form>
          </> : <div className="chat-state">Preparing circle chat…</div>}
      </>}
    {error&&<div className="chat-inline-error" role="alert">{error}<button onClick={()=>setError('')}>Dismiss</button></div>}
    {!online&&<div className="chat-offline" role="status">You are offline. Chat reconnects when the network returns.</div>}
  </section>;
}
