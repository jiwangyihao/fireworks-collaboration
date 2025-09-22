//! Re-exported test event assertion helpers (moved from tests/support for integration tests)
pub use crate::events::structured::{Event, TaskEvent, PolicyEvent, StrategyEvent, TransportEvent, get_global_memory_bus};
use crate::events::structured::{Event as SEvt, TaskEvent as TEvent, PolicyEvent as PEvent, StrategyEvent as SEvent, TransportEvent as TrEvent};

pub fn snapshot_events() -> Vec<SEvt> { get_global_memory_bus().map(|b| b.snapshot()).unwrap_or_default() }
pub fn collect_policy() -> Vec<PEvent> { snapshot_events().into_iter().filter_map(|e| match e { SEvt::Policy(p)=>Some(p), _=>None }).collect() }
pub fn collect_task() -> Vec<TEvent> { snapshot_events().into_iter().filter_map(|e| match e { SEvt::Task(t)=>Some(t), _=>None }).collect() }
pub fn assert_policy_code(code:&str){ let all=collect_policy(); assert!(all.iter().any(|p| matches!(p, PEvent::RetryApplied{code:c,..} if c==code)),"expected policy code={code} got={all:?}") }
pub fn retry_applied_matrix()->Vec<(String,Vec<String>)>{ collect_policy().into_iter().map(|p| match p { PEvent::RetryApplied{id,changed,..}=> (id,changed) }).collect() }
pub fn collect_strategy_summary()->Vec<SEvent>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(s @ SEvent::Summary{..})=>Some(s), _=>None }).collect() }

pub fn collect_transport_partial_fallback()->Vec<(String,bool)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Transport(TrEvent::PartialFilterFallback{id,shallow,..})=>Some((id,shallow)), _=>None }).collect() }
pub fn collect_strategy_conflicts()->Vec<(String,String,String)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(SEvent::Conflict{id,kind,message})=>Some((id,kind,message)), _=>None }).collect() }
pub fn collect_strategy_tls_applied()->Vec<(String,bool,bool)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(SEvent::TlsApplied{id, insecure_skip_verify, skip_san_whitelist, ..})=>Some((id,insecure_skip_verify,skip_san_whitelist)), _=>None }).collect() }
pub fn collect_strategy_http_applied()->Vec<(String,bool,u8)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(SEvent::HttpApplied{id, follow, max_redirects})=>Some((id,follow,max_redirects)), _=>None }).collect() }
pub fn assert_tls_applied(id:&str, expect:bool){ let all=collect_strategy_tls_applied(); let cnt=all.iter().filter(|(tid,_,_)|tid==id).count(); if expect { assert_eq!(cnt,1,"expected exactly one tls applied event for {id} all={all:?}"); } else { assert_eq!(cnt,0,"unexpected tls applied event for {id} all={all:?}"); } }
pub fn assert_http_applied(id:&str, expect:bool){ let all=collect_strategy_http_applied(); let cnt=all.iter().filter(|(tid,_,_)|tid==id).count(); if expect { assert_eq!(cnt,1,"expected exactly one http applied event for {id} all={all:?}"); } else { assert_eq!(cnt,0,"unexpected http applied event for {id} all={all:?}"); } }
pub fn collect_strategy_adaptive_tls()->Vec<(String,String,u8,bool)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(SEvent::AdaptiveTlsRollout{id,kind,percent_applied,sampled})=>Some((id,kind,percent_applied,sampled)), _=>None }).collect() }
pub fn assert_adaptive_tls_event(id:&str, expect:bool){ let all=collect_strategy_adaptive_tls(); let has=all.iter().any(|(tid,_,_,_)|tid==id); assert_eq!(has, expect, "adaptive tls rollout event presence mismatch for {id} all={all:?}"); }
pub fn collect_strategy_ignored_fields()->Vec<(String,String,Vec<String>,Vec<String>)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(SEvent::IgnoredFields{id,kind,top_level,nested})=>Some((id,kind,top_level,nested)), _=>None }).collect() }
pub fn assert_ignored_fields(id:&str, kind:&str, expect_top:&[&str], expect_nested:&[&str]){
	let all=collect_strategy_ignored_fields();
	let hits:Vec<_>=all.into_iter().filter(|(tid,k,_,_)| tid==id && k==kind).collect();
	assert_eq!(hits.len(),1,"expected exactly one IgnoredFields event for {id} kind={kind} got={hits:?}");
	let (_tid,_k,top,nested)=hits.into_iter().next().unwrap();
	for t in expect_top { assert!(top.iter().any(|x| x==t),"expected top-level ignored contains {t} top={top:?}"); }
	for n in expect_nested { assert!(nested.iter().any(|x| x==n),"expected nested ignored contains {n} nested={nested:?}"); }
}
pub fn assert_conflict_kind(id:&str, kind:&str, msg_contains:Option<&str>){ let all=collect_strategy_conflicts(); let hits:Vec<_>=all.iter().filter(|(cid,ck,_)|cid==id&&ck==kind).collect(); assert!(!hits.is_empty(),"expected conflict id={id} kind={kind} got={all:?}"); if let Some(m)=msg_contains { assert!(hits.iter().any(|(_,_,msg)| msg.contains(m)),"expected message contains {m} hits={hits:?}") } }
pub fn summary_applied_codes(id:&str)->Vec<String>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Strategy(SEvent::Summary{id:sid,applied_codes,..}) if sid==id => Some(applied_codes), _=>None }).flatten().collect() }
pub fn assert_applied_code(id:&str, code:&str){ let codes=summary_applied_codes(id); assert!(codes.iter().any(|c|c==code),"expected applied code {code} for {id} got={codes:?}") }
pub fn assert_no_applied_code(id:&str, code:&str){ let codes=summary_applied_codes(id); assert!(!codes.iter().any(|c|c==code),"did not expect applied code {code} for {id} got={codes:?}") }
pub fn collect_transport_partial_capability()->Vec<(String,bool)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Transport(TrEvent::PartialFilterCapability{id,supported})=>Some((id,supported)), _=>None }).collect() }
pub fn collect_transport_partial_unsupported()->Vec<(String,String)>{ snapshot_events().into_iter().filter_map(|e| match e { SEvt::Transport(TrEvent::PartialFilterUnsupported{id,requested})=>Some((id,requested)), _=>None }).collect() }
pub fn assert_partial_capability(id:&str, expect:bool){ let all=collect_transport_partial_capability(); let hits:Vec<_>=all.iter().filter(|(tid,_)|tid==id).collect(); assert!(!hits.is_empty(),"expected capability event for {id}"); assert!(hits.iter().any(|(_,s)|*s==expect),"expected supported={expect} hits={hits:?}") }
pub fn assert_no_partial_capability(id:&str){ let all=collect_transport_partial_capability(); assert!(!all.iter().any(|(tid,_)|tid==id),"unexpected capability event for {id}") }
pub fn assert_partial_unsupported(id:&str, pat:Option<&str>){ let all=collect_transport_partial_unsupported(); let hits:Vec<_>=all.iter().filter(|(tid,_)|tid==id).collect(); assert!(!hits.is_empty(),"expected unsupported event for {id}"); if let Some(p)=pat { assert!(hits.iter().any(|(_,r)|r.contains(p)),"expected requested contains {p} hits={hits:?}") } }
pub fn assert_no_partial_unsupported(id:&str){ let all=collect_transport_partial_unsupported(); assert!(!all.iter().any(|(tid,_)|tid==id),"unexpected unsupported event for {id}") }
pub fn assert_partial_fallback(id:&str, shallow:Option<bool>){ let all=collect_transport_partial_fallback(); let hits:Vec<_>=all.iter().filter(|(tid,_)|tid==id).collect(); assert!(!hits.is_empty(),"expected fallback for {id}"); if let Some(s)=shallow { assert!(hits.iter().any(|(_,sh)|*sh==s),"expected shallow={s} hits={hits:?}") }}
pub fn assert_no_partial_fallback(id:&str){ let all=collect_transport_partial_fallback(); assert!(!all.iter().any(|(tid,_)|tid==id),"unexpected fallback for {id}") }
pub fn task_lifecycle_counters(id:&str)->(usize,usize,usize,usize){ let mut started=0;let mut completed=0;let mut canceled=0;let mut failed=0; for t in collect_task(){ match t { TEvent::Started{id:tid,..} if tid==id=>started+=1, TEvent::Completed{id:tid} if tid==id=>completed+=1, TEvent::Canceled{id:tid} if tid==id=>canceled+=1, TEvent::Failed{id:tid,..} if tid==id=>failed+=1,_=>{} } } (started,completed,canceled,failed) }
#[allow(dead_code)] pub fn debug_dump(){ for e in snapshot_events(){ eprintln!("STRUCTURED_EVENT: {:?}",e); }}
