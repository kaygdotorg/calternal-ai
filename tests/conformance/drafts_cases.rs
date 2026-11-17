//! Every private-input operation and account feed exercised with real sessions.
use super::*;
fn draft(space:&str,item:&str)->Value {json!({"space_id":space,"item_id":item,"text":"private composition"})}
fn queued(space:&str,id:&str,age:u64)->Value {
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    json!({"space_id":space,"item_id":uuid::Uuid::now_v7().to_string(),"created_at_ms":now-age,"request":{"request_id":id,"operation":"notes.create","input":{"space_id":space,"title":id,"markdown":"private pending"}}})
}
async fn account_live(f:&Fixture,who:usize,bookmark:Option<&str>)->reqwest::Response{
    let mut req=f.client.get(format!("{}/api/v1/account/live",f.base)).header("host","localhost:18080").header("origin",ORIGIN).header("cookie",&f.cookies[who]);
    if let Some(b)=bookmark {req=req.query(&[("bookmark",b)]);}
    req.send().await.unwrap()
}
#[tokio::test]
async fn private_input_operations_isolate_accounts_and_hide_revoked_space(){
    let f=Fixture::new().await;let space=&f.spaces[0];let item=uuid::Uuid::now_v7().to_string();let key=json!({"space_id":space,"item_id":item});
    let save=f.op(1,"drafts.save",draft(space,&item)).await;save.ok();
    f.op(1,"drafts.get",key.clone()).await.ok();
    assert_eq!(f.op(2,"drafts.get",key.clone()).await.status,404);
    assert_eq!(f.op(2,"drafts.delete",key.clone()).await.status,404);
    assert_eq!(f.op(2,"drafts.save",draft(space,&item)).await.status,404);
    let other=f.op(2,"drafts.list",json!({})).await;other.ok();assert_eq!(other.body["data"]["records"],json!([]));
    assert!(f.op(2,"drafts.list",json!({"account_id":f.ids[1]})).await.status>=400);
    f.op(1,"pending.queue",queued(space,"private-send",0)).await.ok();
    assert_eq!(f.op(2,"pending.queue",queued(space,"foreign-send",0)).await.status,404);
    let mut edit=queued(space,"private-send",0);
    // Target is fixed by the queued record; edit only the text.
    let list=f.op(1,"pending.list",json!({})).await;list.ok();
    edit["item_id"]=list.body["data"]["records"][0]["item_id"].clone();
    edit["request"]["input"]["markdown"]=json!("edited private pending");
    f.op(1,"pending.edit",edit.clone()).await.ok();assert_eq!(f.op(2,"pending.edit",edit).await.status,404);
    for name in ["pending.cancel","pending.confirm","pending.submit"] {assert_eq!(f.op(2,name,json!({"id":"private-send"})).await.status,404);}
    let other=f.op(2,"pending.list",json!({})).await;other.ok();assert_eq!(other.body["data"]["records"],json!([]));
    f.op(1,"pending.submit",json!({"id":"private-send"})).await.ok();
    f.op(1,"pending.queue",queued(space,"old-send",8*24*60*60*1000)).await.ok();
    let held=f.op(1,"pending.submit",json!({"id":"old-send"})).await;held.ok();assert_eq!(held.body["data"]["record"]["state"],"confirmation_required");
    f.op(1,"pending.confirm",json!({"id":"old-send"})).await.ok();
    f.op(1,"pending.queue",queued(space,"cancel",0)).await.ok();f.op(1,"pending.cancel",json!({"id":"cancel"})).await.ok();
    f.op(1,"drafts.delete",key).await.ok();
    f.op(1,"pending.queue",queued(space,"revoked",0)).await.ok();
    f.op(1,"spaces.delete",json!({"space_id":space})).await.ok();
    assert_eq!(f.op(1,"pending.submit",json!({"id":"revoked"})).await.status,404);
    assert_eq!(f.op(1,"pending.list",json!({})).await.body["data"]["records"],json!([]));
}
#[tokio::test]
async fn account_feed_second_client_restart_foreign_bookmark_and_logout(){
    let mut f=Fixture::new().await;
    let response=account_live(&f,1,None).await;assert_eq!(response.status(),200);
    let mut live=super::live_cases::Live{response,buffer:Vec::new()};
    let ready=live.next().await.unwrap();let bookmark=ready["bookmark"].as_str().unwrap().to_owned();
    let item=uuid::Uuid::now_v7().to_string();f.op(1,"drafts.save",draft(&f.spaces[0],&item)).await.ok();
    assert_eq!(live.next().await.unwrap()["type"],"changed");
    assert_eq!(f.op(1,"drafts.get",json!({"space_id":f.spaces[0],"item_id":item})).await.body["data"]["record"]["text"],"private composition");
    drop(live);f.stop();f.start().await;
    let mut restored=super::live_cases::Live{response:account_live(&f,1,Some(&bookmark)).await,buffer:Vec::new()};assert_eq!(restored.next().await.unwrap()["type"],"changed");
    let mut foreign=super::live_cases::Live{response:account_live(&f,2,Some(&bookmark)).await,buffer:Vec::new()};assert_eq!(foreign.next().await.unwrap()["type"],"reset");
    assert_eq!(f.auth("logout",Some(&f.cookies[1]),json!({})).await.status,204);
    while let Some(frame)=restored.next().await {assert_ne!(frame["type"],"changed");}
    let unauth=f.client.get(format!("{}/api/v1/account/live",f.base)).header("host","localhost:18080").send().await.unwrap();assert_eq!(unauth.status().as_u16(),401);
}
