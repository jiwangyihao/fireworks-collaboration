use async_trait::async_trait;

#[async_trait]
pub trait MyTrait {
    async fn foo(&self) -> u32;
}

struct MyStruct;

#[async_trait]
impl MyTrait for MyStruct {
    async fn foo(&self) -> u32 {
        42
    }
}

#[tokio::test]
async fn test_sanity_mocks() {
    let s = MyStruct;
    assert_eq!(s.foo().await, 42);
}
