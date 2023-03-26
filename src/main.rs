use automerge::{AutoCommit,ObjType,ROOT};
use automerge::transaction::Transactable;
use automerge::ReadDoc;
use automerge::{Value,ObjId,Prop};

struct EthersyncObserver;

impl automerge::OpObserver for EthersyncObserver {
    fn insert<R: ReadDoc>(
        &mut self,
        doc: &R,
        objid: ObjId,
        index: usize,
        tagged_value: (Value<'_>, ObjId)
    ) {}
    fn splice_text<R: ReadDoc>(
        &mut self,
        doc: &R,
        objid: ObjId,
        index: usize,
        value: &str
    ) {
        println!("Splice at index {} with value {}", index, value);
    }
    fn put<R: ReadDoc>(
        &mut self,
        doc: &R,
        objid: ObjId,
        prop: Prop,
        tagged_value: (Value<'_>, ObjId),
        conflict: bool
    ) {}
    fn expose<R: ReadDoc>(
        &mut self,
        doc: &R,
        objid: ObjId,
        prop: Prop,
        tagged_value: (Value<'_>, ObjId),
        conflict: bool
    ) {}
    fn increment<R: ReadDoc>(
        &mut self,
        doc: &R,
        objid: ObjId,
        prop: Prop,
        tagged_value: (i64, ObjId)
    ) {}
    fn delete_map<R: ReadDoc>(&mut self, doc: &R, objid: ObjId, key: &str) {}
    fn delete_seq<R: ReadDoc>(
        &mut self,
        doc: &R,
        objid: ObjId,
        index: usize,
        num: usize
    ) {
        println!("Deleted {} items at index {}", num, index);
    }
}

impl automerge::op_observer::BranchableObserver for EthersyncObserver {
    fn branch(&self) -> Self {
        EthersyncObserver
    }
    fn merge(&mut self, other: &Self) {}
}

impl Clone for EthersyncObserver {
    fn clone(&self) -> Self {
        EthersyncObserver
    }
}

impl EthersyncObserver {
    fn new() -> Self {
        EthersyncObserver
    }
}

fn main() {
    let observer = EthersyncObserver::new();
    let unobserved_doc = AutoCommit::new();
    let mut doc = unobserved_doc.with_observer(observer);
    let t = doc.put_object(ROOT, "text", ObjType::Text).unwrap();

    doc.splice_text(&t, 0, 0, "a").unwrap();
    doc.splice_text(&t, 0, 0, "b").unwrap();

    let s = doc.text(&t).unwrap();
    println!("{:?}", s);

    let mut doc2 = doc.fork();

    doc.splice_text(&t, 1, 0, "1").unwrap();
    doc2.splice_text(&t, 1, 1, "2").unwrap();

    doc.merge(&mut doc2).unwrap();

    let s2 = doc.text(&t).unwrap();
    println!("{:?}", s2);
}
