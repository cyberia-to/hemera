use cyber_hemera::{cdc, commitment as c, cyb_commitment as cyb};

#[test]
fn all_positions_open_in_unbalanced_trees() {
    for n in 1..34 {
        let items: Vec<_> = (0..n).map(|i: u64| c::blob(&i.to_le_bytes())).collect();
        let expected = c::sequence(&items);
        let mut builder = c::SequenceBuilder::default();
        for item in &items {
            builder.push(*item).unwrap();
        }
        assert_eq!(builder.finish(), expected);
        for i in 0..items.len() {
            let (root, proof) = c::prove(&items, i).unwrap();
            assert_eq!(root, expected);
            assert!(c::verify(items[i], &proof, expected));
            assert!(!c::verify(c::blob(b"wrong"), &proof, expected));
            let mut altered = proof.clone();
            altered.count += 1;
            assert!(!c::verify(items[i], &altered, expected));
            altered = proof.clone();
            altered.index = altered.count;
            assert!(!c::verify(items[i], &altered, expected));
            altered = proof.clone();
            altered.siblings[altered.sibling_count] = cyber_hemera::hash(b"extra");
            altered.sibling_count += 1;
            assert!(!c::verify(items[i], &altered, expected));
            if proof.sibling_count > 0 {
                altered = proof.clone();
                altered.siblings[0] = cyber_hemera::hash(b"wrong sibling");
                assert!(!c::verify(items[i], &altered, expected));
                altered.siblings[0] = cyber_hemera::Hash::from_bytes([255; 32]);
                assert!(!c::verify(items[i], &altered, expected));
                altered.sibling_count -= 1;
                assert!(!c::verify(items[i], &altered, expected));
            }
        }
        assert!(c::prove(&items, items.len()).is_none());
    }
}

#[test]
fn domains_cardinality_order_and_empty_are_bound() {
    let a = c::blob(b"a");
    let b = c::blob(b"b");
    assert_ne!(c::sequence(&[a, b]), c::sequence(&[b, a]));
    assert_ne!(c::sequence(&[a]), c::sequence(&[a, a]));
    assert_ne!(c::sequence(&[]), c::sequence(&[c::blob(b"")]));
    assert_ne!(a.as_bytes(), c::sequence(&[a]).as_bytes());
    assert_ne!(c::record(b"one", &[a]), c::record(b"two", &[a]));
    assert_ne!(c::record(b"one", &[a, b]), c::record(b"one", &[b, a]));
    assert!(c::prove(&[], 0).is_none());
    let (_, proof) = c::prove(&[a, b], 1).unwrap();
    let record = c::record(b"one", &[a, b]);
    assert!(c::verify_record_field(b"one", b, &proof, record));
    assert!(!c::verify_record_field(b"two", b, &proof, record));
    assert_eq!(c::ContentId::from_bytes(*a.as_bytes()), Some(a));
    let seq = c::sequence(&[a, b]);
    assert_eq!(c::SequenceId::from_bytes(*seq.as_bytes()), Some(seq));
    assert!(c::ContentId::from_bytes([255; 32]).is_none());
    assert!(c::SequenceId::from_bytes([255; 32]).is_none());
}

#[test]
fn malformed_proofs_with_maximum_counts_fail_without_overflow() {
    let item = c::blob(b"x");
    for count in [0, 1, u64::MAX] {
        for index in [0, 1, u64::MAX - 1, u64::MAX] {
            let proof = c::InclusionProof {
                index,
                count,
                siblings: [cyber_hemera::Hash::from_bytes([0; 32]); 64],
                sibling_count: 0,
            };
            if count == 1 && index == 0 {
                continue;
            }
            assert!(!c::verify(item, &proof, c::sequence(&[item])));
        }
    }
}

#[test]
fn independent_content_survives_preamble_changes_and_reordering() {
    let weights = vec![42; 8192];
    let a = cyb::Entry {
        name: "weights",
        declaration: b"name='weights'\n",
        content: &weights,
        element_size: 32,
    };
    let b = cyb::Entry {
        name: "config",
        declaration: b"name='config'\n",
        content: b"config",
        element_size: 1,
    };
    let first = cyb::container(b"first", &[a, b], &mut [""; 2]).unwrap();
    let second = cyb::container(b"second", &[b, a], &mut [""; 2]).unwrap();
    assert_ne!(first.id, second.id);
    let committed_a = cyb::entry(&a).unwrap();
    let committed_b = cyb::entry(&b).unwrap();
    assert_eq!(
        first.sequence,
        c::sequence(&[committed_a.id, committed_b.id])
    );
    assert_eq!(
        second.sequence,
        c::sequence(&[committed_b.id, committed_a.id])
    );
    let standalone = cyb::section(&weights, 32).unwrap();
    assert_eq!(standalone.id, committed_a.content);
    let chunks: Vec<_> = cdc::chunk_ranges(&weights, 32)
        .unwrap()
        .map(|r| c::blob(&weights[r]))
        .collect();
    assert_eq!(chunks[0], chunks[1]);
    let (root, proof) = c::prove(&chunks, 1).unwrap();
    assert_eq!(root, standalone.sequence);
    assert!(c::verify(chunks[1], &proof, root));
    let renamed = cyb::container(
        b"first",
        &[
            cyb::Entry {
                name: "renamed",
                ..a
            },
            b,
        ],
        &mut [""; 2],
    )
    .unwrap();
    assert_ne!(first.id, renamed.id);
    assert_eq!(
        committed_a.content,
        cyb::entry(&cyb::Entry {
            name: "renamed",
            ..a
        })
        .unwrap()
        .content
    );
}

#[test]
fn rejects_invalid_extractions_and_cdc_layouts() {
    assert!(cyb::section(b"x", 0).is_err());
    assert!(cyb::section(b"x", 65).is_err());
    assert!(cyb::section(b"x", 2).is_err());
    let entry = cyb::Entry {
        name: "x",
        declaration: b"",
        content: b"",
        element_size: 1,
    };
    assert!(cyb::container(b"", &[entry, entry], &mut [""; 2]).is_err());
    assert!(
        cyb::container(
            b"",
            &[cyb::Entry {
                name: "x\ny",
                ..entry
            }],
            &mut [""; 1],
        )
        .is_err()
    );
    assert!(cyb::container(b"", &[entry], &mut []).is_err());
    assert_eq!(c::SequenceBuilder::default().finish(), c::sequence(&[]));
    for size in 1..=64 {
        let bytes = vec![7; size * 143];
        let ranges = cdc::chunk_ranges(&bytes, size).unwrap();
        let mut end = 0;
        for range in ranges {
            assert_eq!(range.start, end);
            assert_eq!(range.start % size, 0);
            assert_eq!(range.end % size, 0);
            assert!(range.end > range.start);
            end = range.end;
        }
        assert_eq!(end, bytes.len());
    }
}
