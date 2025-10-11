//! `测试：utils::event_stream` 的 SSE 解析行为（data 解析为 JSON Value）

use bytes::BytesMut;
use tokio_util::codec::Decoder; // bring decode/decode_eof into scope

#[test]
fn sse_single_event_basic_json() {
    let mut codec = api_proxy::utils::event_stream::EventStreamData::new();
    let mut buf = BytesMut::new();
    buf.extend_from_slice(b"data: {\"a\":1}\n\n");

    let ev = codec.decode(&mut buf).unwrap().expect("one event");
    assert_eq!(ev.event, None);
    assert_eq!(ev.id, None);
    assert_eq!(ev.retry, None);
    let v = ev.data;
    assert_eq!(v.get("a").and_then(serde_json::Value::as_i64), Some(1));
}

#[test]
fn sse_multi_line_data_and_comment_lines() {
    let mut codec = api_proxy::utils::event_stream::EventStreamData::new();
    let mut buf = BytesMut::new();
    // 注释行以冒号开头应被忽略；多行 data 应合并
    buf.extend_from_slice(b": keep-alive\n");
    buf.extend_from_slice(b"data: {\n");
    buf.extend_from_slice(b"data:  \"x\": 42\n");
    buf.extend_from_slice(b"data: }\n\n");

    let ev = codec.decode(&mut buf).unwrap().expect("one event");
    let v = ev.data;
    assert_eq!(v.get("x").and_then(serde_json::Value::as_i64), Some(42));
}

#[test]
fn sse_cross_chunk_and_crlf() {
    let mut codec = api_proxy::utils::event_stream::EventStreamData::new();
    let mut buf1 = BytesMut::new();
    let mut buf2 = BytesMut::new();
    // 第一块：未结束（无空行）
    buf1.extend_from_slice(b"event: delta\r\n");
    buf1.extend_from_slice(b"id: 123\r\n");
    buf1.extend_from_slice(b"data: {\"k\":\"v\"}\r\n");
    assert!(
        codec.decode(&mut buf1).unwrap().is_none(),
        "no complete event yet"
    );

    // 第二块：空行结束事件
    buf2.extend_from_slice(b"\r\n");
    let ev = codec
        .decode(&mut buf2)
        .unwrap()
        .expect("event after boundary");
    assert_eq!(ev.event.as_deref(), Some("delta"));
    assert_eq!(ev.id.as_deref(), Some("123"));
    assert_eq!(ev.data.get("k").and_then(|x| x.as_str()), Some("v"));
}

#[test]
fn sse_done_event_yields_null_data() {
    let mut codec = api_proxy::utils::event_stream::EventStreamData::new();
    let mut buf = BytesMut::new();
    buf.extend_from_slice(b"data: [DONE]\n\n");
    let ev = codec.decode(&mut buf).unwrap().expect("one event");
    assert!(ev.data.is_null(), "[DONE] should produce null data");
}
