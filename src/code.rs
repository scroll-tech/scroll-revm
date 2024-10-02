use primitives::{Address, B256};
use revm::interpreter::StateLoad;

use crate::ScrollWiring;

type KeccakHash = B256;

pub trait ScrollCodeSizeDatabase {
    fn code_size(&self, code_hash: KeccakHash) -> Option<usize>;
}

pub trait ScrollCodeHost {
    fn code_size(&mut self, code_hash: Address) -> Option<(usize, bool)>;
}

impl<EvmWiringT> ScrollCodeHost for revm::Context<EvmWiringT>
where
    EvmWiringT: ScrollWiring,
    EvmWiringT::Database: ScrollCodeSizeDatabase,
{
    fn code_size(&mut self, address: Address) -> Option<(usize, bool)> {
        let StateLoad {
            data: account,
            is_cold,
        } = self.evm.load_account(address).ok()?;
        let code_hash = account.info.code_hash();
        self.evm
            .db
            .code_size(code_hash)
            .map(|code_size| (code_size, is_cold))
    }
}
