# PoC设计方案

[TOC]

## 生成验证方案

### 数据流图

![image](https://user-images.githubusercontent.com/7533730/231416249-93c9f4bf-56b2-4b99-be0e-21461f61c759.png)

生成过程：

Step 1: 若系统处理当前的交易不涉及其他参与方，跳过本步骤，进入Step2。流程图中处理APP2交易时需要关联APP1的输入输出，此时bridge负责串联起该流程。APP1交易完成后，调用APP2交易时透传APP1部分的PoC信息（即APP1的输入和输出）

Step2：APP2在交易完成时会生成并存储PoC内容。生成的过程为将所有关联交易的输入输出分别进行Hash后，将这些Hash用节点私钥签名，之后带上执行节点对应的公钥，一同写入TENET数据库中。存储的索引key值使用APP2的交易ID。

验证过程：

前端验证时，调用获取多个input/output（示例为2个）和调用获取PoC接口后进行验证。验证方法为将所有涉及的输入输出信息两两计算得到对应hash，再加上PoC中存储的sign值还原出公钥，对比PoC中存储的公钥是否一致，如一致则验证通过。

若流程中任意一个阶段数据不匹配，则报错指出。

公开可验证：

PoC对所有人公开，各方可根据交易ID获取PoC内容后，进行相关信息的拉取再还原验证交易的正确性。

可扩展：

开发者基于本框架开发应用时，可按自身系统情况实现getInput和getOutput接口，即可实现多方系统之间的相互验证。

### PoC

```
struct PoC{
    IO_hash_list: []IOHash, // 数组，涉及n方交易则有n个成员
    sign: H256,
    tee_node_public_key: H256, // 该交易执行TEE节点公钥
}
struct IOHash {
    input_hash: H256,
    output_hash: H256,
}
```

PoC存储位置：

每笔交易单独直接以KV形式写入库中。K为当前处理的交易哈希(ID)，V为PoC结构体

### 接口规范

```
interface  {
    H256 getInput(id:H256);
    H256 getOutput(id:H256);
}
```

### 示例实现（以太坊Onchain作为APP1， TENET-APP L2作为APP2）

#### Onchain Input

以太坊getTransaction返回的数据结构进行rlp编码

#### Onchain Output

以太坊getTransactionReceipt返回的数据结构进行rlp编码

#### Offchain Input

```
struct OffChainInput {
    chain_id: u64,
    nonce: U256,
    max_priority_fee_per_gas: U256,
    max_fee_per_gas: U256,
    value: U256,
    input: Bytes,
    access_list: AccessList,
    r: H256,
    s: H256,
}
```

Offchain Input 的值等于以上结构体整体进行rlp编码

#### Offchain Output

```
struct OffChainOutput {
    status_code: u8,
    used_gas: U256,
    logs_bloom: Bloom,
    logs: Vec<Log>,
}
```

Offchain Output的值等于以上结构体整体进行rlp编码

## 附录

### Layer2

```
# frontier中生成以太坊交易哈希使用到的数据结构
pub struct EIP1559Transaction {
    pub chain_id: u64,
    pub nonce: U256,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas_limit: U256,
    pub action: TransactionAction,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    pub odd_y_parity: bool,
    pub r: H256,
    pub s: H256,
}
```

```
# eth_getTransactionByHash接口返回示例
{
  hash: '0x304f3a88c98380d54cf587a6f017db9ed879de13173cfcf5d3f845cf02b82aa6',
  nonce: 0,
  blockHash: '0x0b7fe83712db81aeffb6638de542731ecbb6f565eea99d4d73e35487dc367915',
  blockNumber: 148,
  transactionIndex: 0,
  from: '0x69b91c27a1C644F2D9818DF2320B57BD4e5e0291',
  to: '0x030c1388b6CE0a8D7B1973F5df2d41a1cA4B18f5',
  value: '100000000000000000',
  gasPrice: '1500000008',
  maxFeePerGas: '1500000015',
  maxPriorityFeePerGas: '1500000000',
  gas: 21000,
  input: '0x',
  creates: null,
  raw: '0x02f874820501808459682f008459682f0f82520894030c1388b6ce0a8d7b1973f5df2d41a1ca4b18f588016345785d8a000080c080a00f928862f9d16f4dcbfca93055eb1ed6d1aebdc23bb440fc5bd4b27ad2f5cbe7a06da4684b8fc910f83720692340e320aac86a91c1e06ff25d801a11c3e16f137b',
  publicKey: '0x87dd51effb8408288d3f05a851f7933ffc916bc89d994407bd58db86b2e6a053dc9693cfa24f79200532b93a192afbf10e13650cf174c7a67a1d88648c890b2f',
  chainId: '0x501',
  standardV: '0x0',
  v: '0x0',
  r: '0xf928862f9d16f4dcbfca93055eb1ed6d1aebdc23bb440fc5bd4b27ad2f5cbe7',
  s: '0x6da4684b8fc910f83720692340e320aac86a91c1e06ff25d801a11c3e16f137b',
  accessList: [],
  type: 2
}
```

```
# frontier中receipt数据结构
pub struct EIP658ReceiptData {
    pub status_code: u8,
    pub used_gas: U256,
    pub logs_bloom: Bloom,
    pub logs: Vec<Log>,
}
```

```
# eth_getTransactionReceipt接口返回示例
{
  transactionHash: '0x304f3a88c98380d54cf587a6f017db9ed879de13173cfcf5d3f845cf02b82aa6',
  transactionIndex: 0,
  blockHash: '0x0b7fe83712db81aeffb6638de542731ecbb6f565eea99d4d73e35487dc367915',
  from: '0x69b91c27a1c644f2d9818df2320b57bd4e5e0291',
  to: '0x030c1388b6ce0a8d7b1973f5df2d41a1ca4b18f5',
  blockNumber: 148,
  cumulativeGasUsed: 21000,
  gasUsed: 21000,
  contractAddress: null,
  logs: [],
  logsBloom: '0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
  status: true,
  effectiveGasPrice: 1500000008,
  type: '0x2'
}
```

```
# eth 区块结构
pub struct Header {
    pub parent_hash: H256,
    pub ommers_hash: H256,
    pub beneficiary: H160,
    pub state_root: H256,
    pub transactions_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Bloom,
    pub difficulty: U256,
    pub number: U256,
    pub gas_limit: U256,
    pub gas_used: U256,
    pub timestamp: u64,
    pub extra_data: Bytes,
    pub mix_hash: H256,
    pub nonce: H64,
}
```

```
# getBlock接口返回示例
{
  author: '0x15fdd31c61141abd04a99fd6822c8558854ccde3',
  baseFeePerGas: 8,
  difficulty: '0',
  extraData: '0x',
  gasLimit: 75000000,
  gasUsed: 0,
  hash: '0x7364542fa086d416c0962d4bdb4626d775cf118fbd4758587fb6129d45991405',
  logsBloom: '0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000',
  miner: '0x15fdd31C61141abd04A99FD6822c8558854ccDe3',
  nonce: '0x0000000000000000',
  number: 43905,
  parentHash: '0x05ff33ab5f9f60c6de7a88f90de0dd8c6805c29a9dfb152177c4cd6ec42e421f',
  receiptsRoot: '0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421',
  sha3Uncles: '0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347',
  size: 513,
  stateRoot: '0x14dec7f677548c2c3b56df2426844da587ed1535b897cd43b1c6892e66d90976',
  timestamp: 1680852678,
  totalDifficulty: '0',
  transactions: [],
  transactionsRoot: '0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421',
  uncles: []
}
```

### Layer1

```
# getTransaction
web3.eth.getTransaction('0x9fc76417374aa880d4449a1f7f31ec597f00b1f6f3dd2d66f4c9c6c445836d8b§234')
.then(console.log);
> {
    "hash": "0x9fc76417374aa880d4449a1f7f31ec597f00b1f6f3dd2d66f4c9c6c445836d8b",
    "nonce": 2,
    "blockHash": "0xef95f2f1ed3ca60b048b4bf67cde2195961e0bba6f70bcbea9a2c4e133e34b46",
    "blockNumber": 3,
    "transactionIndex": 0,
    "from": "0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b",
    "to": "0x6295ee1b4f6dd65047762f924ecd367c17eabf8f",
    "value": '123450000000000000',
    "gas": 314159,
    "gasPrice": '2000000000000',
    "input": "0x57cb2fc4"
}
```

```
# getTransactionReceipt
var receipt = web3.eth.getTransactionReceipt('0x9fc76417374aa880d4449a1f7f31ec597f00b1f6f3dd2d66f4c9c6c445836d8b')
.then(console.log);
> {
  "status": true,
  "transactionHash": "0x9fc76417374aa880d4449a1f7f31ec597f00b1f6f3dd2d66f4c9c6c445836d8b",
  "transactionIndex": 0,
  "blockHash": "0xef95f2f1ed3ca60b048b4bf67cde2195961e0bba6f70bcbea9a2c4e133e34b46",
  "blockNumber": 3,
  "contractAddress": "0x11f4d0A3c12e86B4b5F39B213F7E19D048276DAe",
  "cumulativeGasUsed": 314159,
  "gasUsed": 30234,
  "logs": [{
         // logs as returned by getPastLogs, etc.
     }, ...]
}
```
