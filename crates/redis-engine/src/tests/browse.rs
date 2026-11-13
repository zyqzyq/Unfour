use super::*;

#[tokio::test]
async fn hash_browse_preserves_binary_fields_and_continues_an_empty_match() {
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+hash\r\n"),
        exchange(
            &[b"HSCAN", b"k", b"0", b"MATCH", b"p*", b"COUNT", b"50"],
            b"*2\r\n$1\r\n7\r\n*0\r\n",
        ),
        exchange(&[b"TYPE", b"k"], b"+hash\r\n"),
    ])
    .await;
    let page = browse_collection(
        &endpoint,
        None,
        b"k",
        RedisCollectionKind::Hash,
        "0",
        "p*",
        false,
    )
    .await
    .unwrap();
    assert_eq!(page.cursor, "7");
    assert!(page.rows.is_empty());
    server.await.unwrap();
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+hash\r\n"),
        exchange(
            &[b"HSCAN", b"k", b"7", b"MATCH", b"p*", b"COUNT", b"50"],
            b"*2\r\n$1\r\n0\r\n*2\r\n$2\r\np\xff\r\n$0\r\n\r\n",
        ),
    ])
    .await;
    let page = browse_collection(
        &endpoint,
        None,
        b"k",
        RedisCollectionKind::Hash,
        "7",
        "p*",
        false,
    )
    .await
    .unwrap();
    assert_eq!(page.cursor, "0");
    assert_eq!(page.rows[0].member_base64, encoded(b"p\xff"));
    assert_eq!(page.rows[0].value_base64.as_deref(), Some(""));
    server.await.unwrap();
}

#[tokio::test]
async fn range_pages_keep_duplicate_list_elements_and_exact_reverse_scores() {
    let mut list = b"*50\r\n".to_vec();
    for _ in 0..50 {
        list.extend_from_slice(b"$1\r\nx\r\n");
    }
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+list\r\n"),
        exchange(&[b"LRANGE", b"k", b"0", b"49"], &list),
        exchange(&[b"LLEN", b"k"], b":51\r\n"),
    ])
    .await;
    let page = browse_collection(
        &endpoint,
        None,
        b"k",
        RedisCollectionKind::List,
        "0",
        "*",
        false,
    )
    .await
    .unwrap();
    assert_eq!(page.rows.len(), 50);
    assert_eq!(page.rows[49].index, Some(49));
    assert_eq!(page.cursor, "50");
    server.await.unwrap();
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+zset\r\n"),
        exchange(
            &[b"ZREVRANGE", b"k", b"50", b"99", b"WITHSCORES"],
            b"*2\r\n$1\r\n\xff\r\n$17\r\n1.234567890123456\r\n",
        ),
    ])
    .await;
    let page = browse_collection(
        &endpoint,
        None,
        b"k",
        RedisCollectionKind::Zset,
        "50",
        "*",
        true,
    )
    .await
    .unwrap();
    assert_eq!(page.rows[0].index, Some(50));
    assert_eq!(page.rows[0].member_base64, encoded(&[255]));
    assert_eq!(page.rows[0].score.as_deref(), Some("1.234567890123456"));
    assert_eq!(page.cursor, "0");
    server.await.unwrap();
}

#[tokio::test]
async fn range_lookahead_cannot_consume_the_current_pages_byte_budget() {
    // The first 50 sorted-set rows fit in the budget; the next member exceeds
    // the per-item limit. Only a read starting at rank 50 should hit that limit.
    let mut first = b"*100\r\n".to_vec();
    for i in 0..50 {
        first.extend(format!("$2\r\n{i:02}\r\n$1\r\n1\r\n").bytes());
    }
    for (total, cursor) in [(b":51\r\n".as_slice(), "50"), (b":50\r\n", "0")] {
        let (endpoint, server) = scripted(vec![
            exchange(&[b"TYPE", b"k"], b"+zset\r\n"),
            exchange(&[b"ZRANGE", b"k", b"0", b"49", b"WITHSCORES"], &first),
            exchange(&[b"ZCARD", b"k"], total),
        ])
        .await;
        let page = browse_collection(
            &endpoint,
            None,
            b"k",
            RedisCollectionKind::Zset,
            "0",
            "*",
            false,
        )
        .await
        .unwrap();
        assert_eq!(page.cursor, cursor);
        assert_eq!(page.rows.len(), 50);
        server.await.unwrap();
    }
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+zset\r\n"),
        exchange(
            &[b"ZRANGE", b"k", b"50", b"99", b"WITHSCORES"],
            b"*2\r\n$65537\r\n",
        ),
    ])
    .await;
    let error = browse_collection(
        &endpoint,
        None,
        b"k",
        RedisCollectionKind::Zset,
        "50",
        "*",
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "REDIS_RESPONSE_TOO_LARGE");
    server.await.unwrap();
}

#[tokio::test]
async fn scan_pages_bound_bulk_payloads_and_reject_expired_or_retyped_keys() {
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+set\r\n"),
        exchange(
            &[b"SSCAN", b"k", b"0", b"MATCH", b"*", b"COUNT", b"50"],
            b"*2\r\n$1\r\n0\r\n*1\r\n$900000000\r\n",
        ),
    ])
    .await;
    assert_eq!(
        browse_collection(
            &endpoint,
            None,
            b"k",
            RedisCollectionKind::Set,
            "0",
            "*",
            false
        )
        .await
        .unwrap_err()
        .code(),
        "REDIS_RESPONSE_TOO_LARGE"
    );
    server.await.unwrap();
    for kind in [b"+none\r\n".as_slice(), b"+string\r\n"] {
        let (endpoint, server) = scripted(vec![exchange(&[b"TYPE", b"k"], kind)]).await;
        assert_eq!(
            browse_collection(
                &endpoint,
                None,
                b"k",
                RedisCollectionKind::Set,
                "0",
                "*",
                false
            )
            .await
            .unwrap_err()
            .code(),
            "REDIS_KEY_CHANGED"
        );
        server.await.unwrap();
    }
}
