import { StrictMode, useCallback, useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { NearConnector, type NearWalletBase } from '@hot-labs/near-connect';
import { CircleChat } from './CircleChat';
import './styles.css';

const NETWORK = 'testnet' as const;
const RPC = 'https://rpc.testnet.near.org';
const RPC_READ_ENDPOINT = import.meta.env.DEV ? '/api/near-rpc' : RPC;
const CONTRACT = import.meta.env.VITE_CONTRACT_ID || '';
const STORAGE_PRICE_FALLBACK = '10000000000000000000';
const CREATE_STORAGE_BYTES_ESTIMATE = 6000n; // Sandbox measured 4,545 bytes; 20%+ buffer for name and description.
const GAS = '180000000000000';
const YOCTO = 10n ** 24n;

type Circle = { id:number; creator:string; name:string; description:string|null; target_members:number; member_count:number; contribution_yocto:string; security_deposit_yocto:string; payment_interval_ns:string; activated_at_ns:string; first_deadline_ns:string; status:string; current_round:number; successful_payout_count:number; paused_round:number; paused_member_position:number; storage_bytes_funded:number; storage_cost_funded_yocto:string };
type Transfer = { id:number; kind:string; recipient_id:string|null; amount_yocto:string; state:string; attempts:number };
type Member = { account_id:string; payout_position:number; collateral_remaining_yocto:string; total_contributions_yocto:string; total_payouts_yocto:string; on_time_payments:number; late_payments:number; status:string; cancellation_approved:boolean; cancellation_refund_yocto:string; deposit_refund:Transfer; cancellation_refund:Transfer; storage_allowance_refund:Transfer };
type RoundMember = { member_position:number; direct_payment_yocto:string; collateral_substitution_yocto:string; source:string; default_resolution:string };
type Round = { number:number; recipient_position:number; opened_at_ns:string; deadline_ns:string; status:string; total_collected_yocto:string; payout:Transfer; contributions:RoundMember[] };
type Accounting = { member:Member; rounds:RoundMember[] };
type Activity = { hash:string; label:string; time:number };

const connector = new NearConnector({ network: NETWORK, features: { testnet: true }, providers: { testnet: [RPC] }, autoConnect: false });

function utf8Base64(value: string) { const bytes = new TextEncoder().encode(value); let binary = ''; bytes.forEach((byte) => { binary += String.fromCharCode(byte); }); return btoa(binary); }
function decodeBase64Utf8(value: string) { const binary = atob(value); return new TextDecoder().decode(Uint8Array.from(binary, (char) => char.charCodeAt(0))); }
async function rpc<T>(method: string, args: object): Promise<T> {
  if (!CONTRACT) throw new Error('Set VITE_CONTRACT_ID in frontend/.env.local to use contract features.');
  const response = await fetch(RPC_READ_ENDPOINT, { method:'POST', headers:{'content-type':'application/json'}, body:JSON.stringify({jsonrpc:'2.0', id:'kutu', method:'query', params:{request_type:'call_function', finality:'final', account_id:CONTRACT, method_name:method, args_base64:utf8Base64(JSON.stringify(args))}}) });
  const body = await response.json();
  if (!response.ok || body.error) throw new Error(body.error?.cause?.name || body.error?.message || `RPC request failed (${response.status})`);
  const output = new Uint8Array(body.result.result);
  return JSON.parse(new TextDecoder().decode(output)) as T;
}
function yoctoFromNear(input: string) {
  if (!/^\d+(\.\d{1,24})?$/.test(input)) throw new Error('Enter a valid NEAR amount with up to 24 decimal places.');
  const [whole, fraction = ''] = input.split('.');
  return (BigInt(whole) * YOCTO + BigInt((fraction + '0'.repeat(24)).slice(0, 24))).toString();
}
function near(value: string) { const amount=BigInt(value || '0'); const whole=amount/YOCTO; const fraction=(amount%YOCTO).toString().padStart(24,'0').replace(/0+$/,''); return `${whole}${fraction ? `.${fraction.slice(0,6)}` : ''} NEAR`; }
function dateFromNs(ns: string) { const ms=Number(BigInt(ns)/1_000_000n); return ms ? new Date(ms).toLocaleString() : 'Not active'; }
function shortHash(hash: string) { return `${hash.slice(0,9)}…${hash.slice(-6)}`; }
function errorText(error: unknown) { const message=error instanceof Error ? error.message : String(error); return message.replaceAll('_',' ').replace(/ExecutionError: ?/g,'').replace(/Smart contract panicked: ?/g,''); }

function App() {
  const [wallet, setWallet] = useState<NearWalletBase|null>(null);
  const [account, setAccount] = useState('');
  const [circleId, setCircleId] = useState('');
  const [circle, setCircle] = useState<Circle|null>(null);
  const [members, setMembers] = useState<Member[]>([]);
  const [round, setRound] = useState<Round|null>(null);
  const [accounting, setAccounting] = useState<Accounting|null>(null);
  const [storagePrice, setStoragePrice] = useState(STORAGE_PRICE_FALLBACK);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [activeTab, setActiveTab] = useState<'overview'|'activity'>('overview');
  const [activity, setActivity] = useState<Activity[]>(() => JSON.parse(localStorage.getItem('kutu:testnet:activity') || '[]') as Activity[]);
  const [recent, setRecent] = useState<string[]>(() => JSON.parse(localStorage.getItem('kutu:testnet:circles') || '[]') as string[]);
  const [create, setCreate] = useState({ name:'', description:'', contribution:'0.1', collateral:'0.1', members:'2', interval:'86400' });
  const [joinBudget, setJoinBudget] = useState('0.01');

  const refresh = useCallback(async (id = circleId, who = account) => {
    if (!id || !/^\d+$/.test(id)) return;
    setError('');
    const parsed=Number(id);
    const [nextCircle,nextMembers,nextPrice]=await Promise.all([
      rpc<Circle>('get_circle',{circle_id:parsed}), rpc<Member[]>('get_members',{circle_id:parsed}), rpc<string>('get_storage_price_yocto',{}).catch(()=>STORAGE_PRICE_FALLBACK),
    ]);
    setCircle(nextCircle); setMembers(nextMembers); setStoragePrice(nextPrice);
    const roundResult=await rpc<Round>('get_round',{circle_id:parsed,round_number:nextCircle.current_round}).catch(()=>null);
    setRound(roundResult);
    if (who && nextMembers.some((member)=>member.account_id===who)) setAccounting(await rpc<Accounting>('get_member_accounting',{circle_id:parsed,account_id:who}));
    else setAccounting(null);
    const ids=[id,...recent.filter((item)=>item!==id)].slice(0,12); setRecent(ids); localStorage.setItem('kutu:testnet:circles',JSON.stringify(ids));
  },[account,circleId,recent]);

  useEffect(()=> { void rpc<string>('get_storage_price_yocto',{}).then(setStoragePrice).catch(()=>{}); void connector.getConnectedWallet().then(async({wallet:connected})=>{ const accounts=await connected.getAccounts({network:NETWORK});if(accounts[0]){setWallet(connected);setAccount(accounts[0].accountId);} }).catch(()=>{}); },[]);
  useEffect(()=> { if (circleId) void refresh(circleId).catch((e)=>setError(errorText(e))); },[circleId,refresh]);
  useEffect(()=> { if (!wallet) return; const update=(payload:{wallet:NearWalletBase;accounts:{accountId:string}[]})=>{setWallet(payload.wallet);setAccount(payload.accounts[0]?.accountId||'');}; const clear=()=>{setWallet(null);setAccount('');setAccounting(null);}; connector.on('wallet:signIn',update); connector.on('wallet:signOut',clear); return ()=>{connector.off('wallet:signIn',update);connector.off('wallet:signOut',clear);}; },[wallet]);

  const isMember=!!account&&members.some((member)=>member.account_id===account);
  const ownMember=members.find((member)=>member.account_id===account);
  const ownRound=round?.contributions.find((item)=>item.member_position===ownMember?.payout_position);
  const recipient=members.find((member)=>member.payout_position===round?.recipient_position);
  const paidCount=round?.contributions.filter((item)=>BigInt(item.direct_payment_yocto)>0n||BigInt(item.collateral_substitution_yocto)>0n).length||0;
  const joinDeposit=useMemo(()=> {try{return (BigInt(circle?.security_deposit_yocto||'0')+BigInt(yoctoFromNear(joinBudget))).toString();}catch{return '0';}},[circle?.security_deposit_yocto,joinBudget]);

  const rememberTx=(hash:string,label:string)=> {const items=[{hash,label,time:Date.now()},...activity].slice(0,40);setActivity(items);localStorage.setItem('kutu:testnet:activity',JSON.stringify(items));};
  async function connect() { setBusy(true);setError('');try { const selected=await connector.connect();const accounts=await selected.getAccounts({network:NETWORK});if(!accounts[0])throw new Error('The wallet did not return a Testnet account.');setWallet(selected);setAccount(accounts[0].accountId);setNotice('Testnet wallet connected.'); }catch(e){setError(errorText(e));}finally{setBusy(false);} }
  async function disconnect() {try{await wallet?.signOut({network:NETWORK});await connector.disconnect(wallet||undefined);}catch(e){setError(errorText(e));}setWallet(null);setAccount('');setAccounting(null);}
  async function transact(method:string,args:object,deposit='0',label=method) {if(!wallet||!account)throw new Error('Connect a NEAR Testnet wallet first.');if(!CONTRACT)throw new Error('Set VITE_CONTRACT_ID in frontend/.env.local first.');setBusy(true);setError('');setNotice('Review and approve the transaction in your wallet.');try{const outcome=await wallet.signAndSendTransaction({network:NETWORK,signerId:account,receiverId:CONTRACT,actions:[{type:'FunctionCall',params:{methodName:method,args,gas:GAS,deposit}}]});const status=outcome.status as {SuccessValue?:string;Failure?:unknown};if(status.Failure)throw new Error(JSON.stringify(status.Failure));const hash=outcome.transaction?.hash||'';if(hash)rememberTx(hash,label);setNotice(`${label} confirmed on NEAR Testnet${hash?` · ${shortHash(hash)}`:''}.`);await refresh(circleId,account);return status.SuccessValue?decodeBase64Utf8(status.SuccessValue):'';}catch(e){throw new Error(errorText(e));}finally{setBusy(false);}}
  async function run(action:()=>Promise<unknown>) {try{await action();}catch(e){setError(errorText(e));setNotice('');}}
  async function createCircle() {
    const contribution=yoctoFromNear(create.contribution);
    const collateral=yoctoFromNear(create.collateral);
    const interval=BigInt(create.interval)*1_000_000_000n;
    if(!create.name.trim())throw new Error('Add a circle name.');
    if(new TextEncoder().encode(create.name).length>64||new TextEncoder().encode(create.description).length>280)throw new Error('Name and description limits are measured in UTF-8 bytes.');
    if(!Number.isInteger(Number(create.members))||Number(create.members)<2||Number(create.members)>10)throw new Error('Choose a target between 2 and 10 members.');
    if(interval<60_000_000_000n||interval>31_536_000_000_000_000n)throw new Error('Payment interval must be from 60 seconds to 365 days.');
    const intervalNumber=Number(interval);
    if(BigInt(intervalNumber)!==interval)throw new Error('Payment interval cannot be represented exactly as a JSON number.');
    const storageDeposit=(BigInt(storagePrice)*CREATE_STORAGE_BYTES_ESTIMATE).toString();
    const value=await transact('create_circle',{args:{name:create.name,description:create.description||null,max_members:Number(create.members),contribution_yocto:contribution,security_deposit_yocto:collateral,payment_interval_ns:intervalNumber}},storageDeposit,'Create circle');
    const id=decodeCircleId(value);
    setCircleId(String(id));
    setNotice(`Circle #${id} created. Join it explicitly to take a payout position.`);
  }
  function decodeCircleId(base64:string) {const decoded=decodeBase64Utf8(base64);try{return Number(JSON.parse(decoded) as string|number);}catch{return Number(decoded);}}
  async function doJoin() {if(!circle)throw new Error('Load a circle first.');const storage=yoctoFromNear(joinBudget);const deposit=(BigInt(circle.security_deposit_yocto)+BigInt(storage)).toString();await transact('join_circle',{circle_id:circle.id},deposit,'Join circle');}
  async function doExactPayment(method:string,label:string) {if(!circle)throw new Error('Load a circle first.');const amount=circle.contribution_yocto;await transact(method,{circle_id:circle.id},amount,label);}
  function transferSlot(t:Transfer, memberPosition?:number, roundNumber?:number) {if(t.kind==='Payout')return {RoundPayout:roundNumber};if(t.kind==='DepositRefund')return {MemberDepositRefund:memberPosition};if(t.kind==='CancellationRefund')return {MemberCancellationRefund:memberPosition};if(t.kind==='StorageAllowanceRefund')return memberPosition?{MemberStorageRefund:memberPosition}:'CircleStorageRefund';throw new Error('Unknown transfer type.');}
  async function retry(t:Transfer, memberPosition?:number, roundNumber?:number) {if(!circle) return;await transact('retry_transfer',{circle_id:circle.id,slot:transferSlot(t,memberPosition,roundNumber),transfer_id:t.id},'0','Retry transfer');}
  const action=(method:string,args:object={},label=method)=>run(()=>transact(method,{circle_id:circle!.id,...args},'0',label));

  return <div className="app-shell">
    <header className="topbar"><a className="brand" href="#top"><span className="brand-mark">k</span><span>KUTU<span className="brand-light">/</span>DAO</span></a><nav><a href="#circles">Circles</a><a href="#how-it-works">How it works</a></nav><div className="wallet-area">{account?<><span className="network-pill"><i/>NEAR TESTNET</span><span className="account-chip">{account}</span><button className="button button-light button-small" onClick={()=>void disconnect()}>Disconnect</button></>:<button className="button button-dark button-small" disabled={busy} onClick={()=>void connect()}>Connect wallet <span>↗</span></button>}</div></header>
    <main id="top">
      <section className="hero"><div className="hero-copy"><div className="eyebrow"><span className="eyebrow-line"/>COMMUNITY SAVINGS, ON-CHAIN</div><h1>Good things<br/>grow <em>together.</em></h1><p>A simple, transparent way for people to save together. Take turns receiving the pot, with every contribution and payout visible on NEAR.</p><div className="hero-actions"><a className="button button-dark" href="#create">Start a circle <span>↗</span></a><a className="text-link" href="#circles">Explore a circle <span>↓</span></a></div><div className="hero-foot"><span className="pulse"/> LIVE ON NEAR TESTNET <span className="foot-dot">·</span> NON-CUSTODIAL BY DESIGN</div></div><div className="hero-art" aria-label="Illustration of a rotating savings circle"><div className="art-sun"/><div className="art-ring art-ring-one"/><div className="art-ring art-ring-two"/><div className="art-center"><span>the pot</span><strong>{circle?near((BigInt(circle.contribution_yocto)*BigInt(circle.target_members)).toString()):'— NEAR'}</strong><small>{circle?`${circle.target_members} people · one shared goal`:'ONE CIRCLE, MANY POSSIBILITIES'}</small></div><div className="orbit-tag tag-one"><span className="avatar avatar-peach">A</span><span>save a little<br/><b>every round</b></span></div><div className="orbit-tag tag-two"><span className="avatar avatar-sage">B</span><span>take turns<br/><b>getting the pot</b></span></div><div className="orbit-tag tag-three"><span className="avatar avatar-lilac">C</span><span>keep it clear<br/><b>on-chain</b></span></div><div className="spark spark-a">✳</div><div className="spark spark-b">✦</div></div></section>
      <section className="ticker"><span>OPEN MEMBERSHIP</span><b>✳</b><span>FIXED PAYOUT ORDER</span><b>✳</b><span>ON-CHAIN TRANSPARENCY</span><b>✳</b><span>NEAR TESTNET</span></section>
      {error&&<div className="message message-error" role="alert"><span>!</span><div><b>Something needs attention</b><p>{error}</p></div><button onClick={()=>setError('')} aria-label="Dismiss">×</button></div>}{notice&&<div className="message message-success" role="status"><span>✓</span><p>{notice}</p><button onClick={()=>setNotice('')} aria-label="Dismiss">×</button></div>}
      <section className="section create-section" id="create"><div className="section-heading"><div><div className="eyebrow"><span className="eyebrow-line"/>MAKE IT YOURS</div><h2>A circle that<br/><em>fits your people.</em></h2></div><p>Pick the contribution, set the pace, and invite the people you trust. The rules are fixed once the circle starts.</p></div><div className="create-layout"><div className="create-card"><div className="card-top"><div><span className="step-number">01</span><h3>Set up your circle</h3></div><span className="lock-label">RULES LOCK AT START</span></div><div className="form-grid"><label className="field field-wide">Circle name<input maxLength={64} value={create.name} onChange={(e)=>setCreate({...create,name:e.target.value})} placeholder="e.g. Sunday supper club"/><small>{new TextEncoder().encode(create.name).length}/64 UTF-8 bytes</small></label><label className="field field-wide">A short description <span>optional</span><textarea maxLength={280} value={create.description} onChange={(e)=>setCreate({...create,description:e.target.value})} placeholder="What are you saving toward?" rows={2}/><small>{new TextEncoder().encode(create.description).length}/280 UTF-8 bytes</small></label><label className="field">Contribution each round<div className="input-unit"><input inputMode="decimal" value={create.contribution} onChange={(e)=>setCreate({...create,contribution:e.target.value})}/><span>NEAR</span></div></label><label className="field">Security deposit per member<div className="input-unit"><input inputMode="decimal" value={create.collateral} onChange={(e)=>setCreate({...create,collateral:e.target.value})}/><span>NEAR</span></div></label><label className="field">Member target<select value={create.members} onChange={(e)=>setCreate({...create,members:e.target.value})}>{Array.from({length:9},(_,i)=>i+2).map((n)=><option key={n} value={n}>{n} members</option>)}</select></label><label className="field">Time to contribute · seconds<input type="number" min="60" max="31536000" step="1" value={create.interval} onChange={(e)=>setCreate({...create,interval:e.target.value})}/><small>60 seconds to 365 days</small></label></div><div className="storage-note"><span className="storage-icon">◈</span><div><b>Separate storage funding</b><p>Creation attaches the current Testnet storage price × 6,000 bytes. Sandbox measured 4,545 bytes for circle creation; the extra room covers maximum text fields. The contract measures actual storage and returns excess. Occupied storage is not refundable.</p></div></div><button className="button button-dark create-submit" disabled={busy||!account||!CONTRACT} onClick={()=>void run(createCircle)}>Review & create circle <span>↗</span></button>{!CONTRACT&&<p className="setup-hint">Add your deployed Testnet contract ID to <code>frontend/.env.local</code> to enable transactions.</p>}</div><aside className="aside-note"><div className="note-index">A NOTE ON THE ORDER</div><div className="order-visual"><div className="order-row"><span>01</span><i className="order-dot dot-filled"/><div><b>First to join</b><small>receives the first pot</small></div><span className="order-arrow">↗</span></div><div className="order-row"><span>02</span><i className="order-dot"/><div><b>Next to join</b><small>receives the second pot</small></div></div><div className="order-row"><span>03</span><i className="order-dot"/><div><b>And so on</b><small>one turn each</small></div></div></div><div className="aside-rule"/><p>You’ll still need to join your own circle after creating it. Every member earns a permanent payout position in the order they successfully join.</p></aside></div></section>
      <section className="section circles-section" id="circles"><div className="section-heading compact"><div><div className="eyebrow"><span className="eyebrow-line"/>YOUR CIRCLE SPACE</div><h2>Find your <em>people.</em></h2></div><p>Open a circle by its ID. Share the ID with members so they can join from their own wallet.</p></div><div className="circle-finder"><span className="finder-icon">⌕</span><input aria-label="Circle ID" value={circleId} onChange={(e)=>setCircleId(e.target.value.replace(/\D/g,''))} placeholder="Enter a circle ID" onKeyDown={(e)=>e.key==='Enter'&&void refresh()}/><button className="button button-dark button-small" disabled={!circleId||busy} onClick={()=>void refresh()}>Open circle <span>↗</span></button></div>{recent.length>0&&<div className="recent-row"><span>RECENT</span>{recent.map((id)=><button key={id} className={id===circleId?'recent-chip selected':'recent-chip'} onClick={()=>setCircleId(id)}>#{id}</button>)}</div>}
      {circle?<div className="dashboard"><div className="dashboard-head"><div><span className={`status-tag status-${circle.status.toLowerCase()}`}><i/>{circle.status}</span><h3>{circle.name}</h3><p>{circle.description||'A community savings circle'} <span>·</span> Circle #{circle.id}</p></div><div className="dashboard-actions">{account&&!isMember&&circle.status==='Open'&&<button className="button button-dark" disabled={busy} onClick={()=>void run(doJoin)}>Join circle <span>↗</span></button>}{isMember&&circle.status==='Open'&&<button className="button button-dark" disabled={busy} onClick={()=>void run(doJoin)}>Join as a member <span>↗</span></button>}<button className="button button-light button-small" disabled={busy} onClick={()=>void refresh()}>↻ Refresh</button></div></div>
      <div className="join-storage"><label>Extra storage allowance<div className="input-unit"><input inputMode="decimal" value={joinBudget} onChange={(e)=>setJoinBudget(e.target.value)}/><span>NEAR</span></div></label><p>Join attaches {near(joinDeposit)} total: collateral plus this allowance. The contract charges storage at {near(storagePrice)} per byte and returns any excess. Choose an allowance that covers actual storage use.</p></div><div className="stats-grid"><Stat label="Contribution / round" value={near(circle.contribution_yocto)}/><Stat label="Members" value={`${circle.member_count} / ${circle.target_members}`}/><Stat label="Payment window" value={formatInterval(circle.payment_interval_ns)}/><Stat label="Round" value={circle.status==='Completed'?'Complete':`${circle.current_round+1} of ${circle.target_members}`}/></div>
      <div className="dashboard-columns"><div className="panel"><div className="panel-heading"><div><span className="panel-kicker">THE PEOPLE</span><h4>Payout order</h4></div><span className="member-count">{circle.member_count} members</span></div><div className="member-list">{members.map((member)=>{const row=round?.contributions.find((item)=>item.member_position===member.payout_position);return <div className={`member-row ${member.account_id===account?'member-you':''}`} key={member.account_id}><span className="position">{String(member.payout_position).padStart(2,'0')}</span><span className={`avatar mini avatar-${['peach','sage','lilac','blue'][member.payout_position%4]}`}>{member.account_id.slice(0,1).toUpperCase()}</span><div className="member-main"><b>{member.account_id}{member.account_id===account&&<em>you</em>}</b><small>{member.payout_position===round?.recipient_position?'Payout recipient':`Payout position ${member.payout_position}`}</small></div><div className="member-funds"><b>{row&&(BigInt(row.direct_payment_yocto)>0n||BigInt(row.collateral_substitution_yocto)>0n)?'Ready':'Awaiting'}</b><small>Collateral {near(member.collateral_remaining_yocto)}</small></div></div>})}{members.length<circle.target_members&&<div className="open-slot"><span>＋</span>{circle.target_members-members.length} {circle.target_members-members.length===1?'place':'places'} left · share circle #{circle.id}</div>}</div></div>
      <div className="panel round-panel"><div className="panel-heading"><div><span className="panel-kicker">ROUND {circle.current_round+1}</span><h4>{round?.status||'Waiting for members'}</h4></div><span className="round-counter">{paidCount} / {circle.target_members} paid</span></div><div className="pot-card"><span>THIS ROUND’S POT</span><strong>{near(round?.total_collected_yocto||'0')}</strong><small>{recipient?`To ${recipient.account_id}`:`Recipient position ${round?.recipient_position||1}`}</small></div><div className="deadline-line"><span>◷</span><div><b>{circle.status==='Open'?'Starts when the circle fills':`Deadline ${dateFromNs(round?.deadline_ns||circle.first_deadline_ns)}`}</b><small>{circle.status==='Open'?'The countdown begins at activation':`Window: ${formatInterval(circle.payment_interval_ns)}`}</small></div></div>{account&&isMember&&circle.status==='Active'&&(round?.status==='Collecting'||round?.status==='ReadyForPayout')&&ownRound&&BigInt(ownRound.direct_payment_yocto)===0n&&BigInt(ownRound.collateral_substitution_yocto)===0n&&<button className="button button-dark full-button" disabled={busy} onClick={()=>void run(()=>doExactPayment('contribute','Contribute to round'))}>Contribute exactly {near(circle.contribution_yocto)} <span>↗</span></button>}{circle.status==='Active'&&round?.status==='Collecting'&&circle.member_count===circle.target_members&&<button className="button button-light full-button" disabled={busy} onClick={()=>void action('process_defaults',{},'Process overdue members')}>Check overdue contributions</button>}{circle.status==='Active'&&round?.status==='ReadyForPayout'&&<button className="button button-dark full-button" disabled={busy} onClick={()=>void action('finalize_payout',{},'Finalize payout')}>Finalize payout · permissionless <span>↗</span></button>}{round&&round.payout.state==='Failed'&&<TransferRetry transfer={round.payout} busy={busy} onRetry={()=>void run(()=>retry(round.payout,undefined,round.number))}/>}</div></div>
      {circle.status==='Paused'&&<div className="paused-banner"><span>Ⅱ</span><div><b>Circle paused at round {circle.paused_round+1}</b><p>Member at payout position {circle.paused_member_position} may cure by paying the exact contribution. No payout or refund will proceed until resolved.</p></div>{account&&ownMember?.payout_position===circle.paused_member_position&&<button className="button button-dark button-small" disabled={busy} onClick={()=>void run(()=>doExactPayment('cure_default','Cure missed contribution'))}>Pay exact cure <span>↗</span></button>}</div>}
      <div className="dashboard-bottom">
        <div className="panel history-panel">
          <div className="panel-heading tabs-heading"><div className="tabs">
            <button className={activeTab==='overview'?'active':''} onClick={()=>setActiveTab('overview')}>Overview</button>
            <button className={activeTab==='activity'?'active':''} onClick={()=>setActiveTab('activity')}>Activity</button>
          </div></div>
          {activeTab==='overview'?<>
            <div className="accounting-grid"><Metric label="Collateral remaining" value={accounting?near(accounting.member.collateral_remaining_yocto):'Connect as a member'}/><Metric label="My contributions" value={accounting?near(accounting.member.total_contributions_yocto):'—'}/><Metric label="My payouts" value={accounting?near(accounting.member.total_payouts_yocto):'—'}/><Metric label="Cancellation consent" value={ownMember?.cancellation_approved?'Approved':'Not approved'}/></div>
            {accounting&&<div className="my-rounds"><b>My round-by-round contributions</b>{accounting.rounds.map((item,index)=><div key={index}><span>Round {index+1}</span><span>{near(item.direct_payment_yocto)} direct · {near(item.collateral_substitution_yocto)} collateral</span><small>{item.default_resolution.replaceAll('_',' ')}</small></div>)}</div>}
          </>:<ActivityList activity={activity}/>}
        </div>
        <div className="panel action-panel">
          <span className="panel-kicker">CIRCLE ACTIONS</span><h4>More ways to take part</h4>
          <div className="action-stack">
            {ownMember&&[ownMember.storage_allowance_refund,ownMember.deposit_refund,ownMember.cancellation_refund].filter((transfer)=>transfer.state==='Failed').map((transfer)=><button key={`${transfer.kind}-${transfer.id}`} disabled={busy} onClick={()=>void run(()=>retry(transfer,ownMember.payout_position))}>Retry failed {transfer.kind.toLowerCase()} · {near(transfer.amount_yocto)} (attempt {transfer.attempts})</button>)}
            {ownMember&&circle.successful_payout_count===0&&circle.status!=='Cancelled'&&<button disabled={busy||ownMember.cancellation_approved} onClick={()=>void action('approve_cancellation',{},'Approve cancellation')}>{ownMember.cancellation_approved?'✓ Cancellation approved':'Approve cancellation'}</button>}
            {ownMember&&ownMember.cancellation_approved&&circle.successful_payout_count===0&&circle.status!=='Cancelled'&&<button disabled={busy} onClick={()=>void action('revoke_cancellation',{},'Revoke my consent')}>Revoke my consent</button>}
            {circle.status!=='Cancelled'&&circle.status!=='Completed'&&circle.successful_payout_count===0&&circle.member_count>0&&<button disabled={busy} onClick={()=>void action('finalize_cancellation',{},'Finalize unanimous cancellation')}>Finalize cancellation if unanimous</button>}
            {circle.status==='Cancelled'&&ownMember&&BigInt(ownMember.cancellation_refund_yocto)>0n&&<button disabled={busy} onClick={()=>void action('claim_cancellation_refund',{position:ownMember.payout_position},'Claim cancellation refund')}>Claim refund · {near(ownMember.cancellation_refund_yocto)}</button>}
            {circle.status==='Completed'&&ownMember&&BigInt(ownMember.collateral_remaining_yocto)>0n&&<button disabled={busy} onClick={()=>void action('claim_deposit_refund',{},'Claim collateral refund')}>Claim collateral refund</button>}
            {circle.status==='Open'&&circle.member_count===0&&circle.creator===account&&<button disabled={busy} onClick={()=>void action('close_empty_circle',{},'Close empty circle')}>Close this empty circle</button>}
            <span>Only the contract can decide whether an action is valid.</span>
          </div>
        </div>
      </div>
      <CircleChat key={circle.id} circleId={String(circle.id)} account={account} wallet={wallet}/>
      </div>:<div className="empty-state"><div className="empty-symbol">◎</div><h3>Your circle will show up here.</h3><p>Open one with its ID, or create one above. No off-chain balance is treated as authoritative.</p></div>}
      </section>
      <section className="section how-section" id="how-it-works"><div className="section-heading compact"><div><div className="eyebrow"><span className="eyebrow-line"/>SMALL STEPS, SHARED MOMENTUM</div><h2>How a circle <em>turns.</em></h2></div><p>Every rule is written into the contract. The community decides to join; the code keeps the books.</p></div><div className="steps-grid"><Step number="01" title="Start together" text="Create a circle, share its ID, and let people join. Your place in line is the order you join."/><Step number="02" title="Put in your share" text="Every member contributes the same exact amount during each round’s window."/><Step number="03" title="Take your turn" text="Once everyone has paid—or eligible collateral covers a missed share—the scheduled member receives the pot."/><Step number="04" title="Keep it moving" text="The next round opens after a successful payout. If there is a shortfall, the circle pauses safely."/></div></section>
      <section className="trust-band"><div><span className="eyebrow"><span className="eyebrow-line"/>BUILT FOR CLARITY</span><h2>Your circle.<br/><em>Your people.</em></h2></div><div className="trust-points"><p><span>01</span><b>Rules set in code</b><small>Circle terms can’t be changed after activation.</small></p><p><span>02</span><b>Every amount accounted for</b><small>Collateral, contributions and storage stay separate.</small></p><p><span>03</span><b>No back-room withdrawals</b><small>Only predetermined payouts and contract-defined refunds.</small></p></div><div className="trust-stamp">✳<small>ON-CHAIN<br/>BY DESIGN</small></div></section>
    </main><footer><a className="brand footer-brand" href="#top"><span className="brand-mark">k</span><span>KUTU<span className="brand-light">/</span>DAO</span></a><span>Community savings, built in the open.</span><span className="footer-network"><i/> NEAR TESTNET ONLY</span></footer>
    {busy&&<div className="busy-overlay"><div className="busy-box"><span className="spinner"/><b>Waiting for the chain</b><small>Your wallet may ask you to review this action.</small></div></div>}
  </div>;
}

function formatInterval(ns:string) {const seconds=Number(BigInt(ns)/1_000_000_000n);if(seconds%86400===0)return `${seconds/86400} ${seconds===86400?'day':'days'}`;if(seconds%3600===0)return `${seconds/3600} ${seconds===3600?'hour':'hours'}`;if(seconds%60===0)return `${seconds/60} minutes`;return `${seconds} seconds`;}
function Stat({label,value}:{label:string;value:string}){return <div className="stat-card"><span>{label}</span><b>{value}</b></div>}
function Metric({label,value}:{label:string;value:string}){return <div className="metric"><span>{label}</span><b>{value}</b></div>}
function Step({number,title,text}:{number:string;title:string;text:string}){return <article className="step-card"><span>{number}</span><h3>{title}</h3><p>{text}</p></article>}
function TransferRetry({transfer,busy,onRetry}:{transfer:Transfer;busy:boolean;onRetry:()=>void}){return <div className="transfer-failed"><span>↻</span><div><b>Transfer failed · attempt {transfer.attempts}</b><small>{near(transfer.amount_yocto)} entitlement is still recorded for {transfer.recipient_id||'the saved recipient'}.</small></div><button disabled={busy} onClick={onRetry}>Retry transfer</button></div>}
function ActivityList({activity}:{activity:Activity[]}) {return activity.length?<div className="activity-list">{activity.map((item)=><a key={item.hash} href={`https://testnet.nearblocks.io/txns/${item.hash}`} target="_blank" rel="noreferrer"><span className="activity-check">✓</span><div><b>{item.label}</b><small>{new Date(item.time).toLocaleString()}</small></div><code>{shortHash(item.hash)}</code><span>↗</span></a>)}</div>:<div className="activity-empty">Confirmed actions from this browser will appear here. For full history, use the Testnet Explorer link on each transaction.</div>}

createRoot(document.getElementById('root')!).render(<StrictMode><App/></StrictMode>);
