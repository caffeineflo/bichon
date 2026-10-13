use bichon_blob::{Codec, Config, Engine};
use tempfile::TempDir;

#[test]
fn compacted_survivors_remain_readable_after_reopen() {
    let dir = TempDir::new().unwrap();
    let value = vec![0x42; 5000];
    {
        let engine = Engine::open(dir.path(), Config::default()).unwrap();
        for index in 0..6u8 {
            engine.put([index; 32], &value, Codec::None).unwrap();
        }
        engine.delete(&[0; 32]).unwrap();
        engine.delete(&[1; 32]).unwrap();
        engine.seal_active_segment().unwrap();
        assert!(engine.gc().unwrap().is_some());
        engine.shutdown().unwrap();
    }
    let engine = Engine::open(dir.path(), Config::default()).unwrap();
    let survivors: Vec<_> = (2..6u8)
        .map(|index| engine.get(&[index; 32]).unwrap())
        .collect();
    assert_eq!(survivors, vec![Some(value); 4]);
}
