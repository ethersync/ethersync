use automerge::{AutoCommit,ObjType,ROOT};
use automerge::transaction::Transactable;
use automerge::ReadDoc;

fn main() {
    let mut doc = AutoCommit::new();
    let t = doc.put_object(ROOT, "text", ObjType::Text).unwrap();

    doc.splice_text(&t, 0, 0, "a").unwrap();
    doc.splice_text(&t, 0, 0, "b").unwrap();

    let s = doc.text(&t).unwrap();
    println!("{:?}", s);

    let mut doc2 = doc.fork();

    doc.splice_text(&t, 0, 0, "1").unwrap();
    doc2.splice_text(&t, 0, 0, "2").unwrap();

    doc.merge(&mut doc2).unwrap();

    let s2 = doc.text(&t).unwrap();
    println!("{:?}", s2);
}
