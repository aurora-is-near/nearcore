use std::slice;

/// Protocol configuration parameter which may change between protocol versions.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::IntoStaticStr,
    strum::EnumString,
    Debug,
    strum::Display,
)]
#[strum(serialize_all = "snake_case")]
pub enum Parameter {
    // Gas economics config
    BurntGasRewardNumerator,
    BurntGasRewardDenominator,
    PessimisticGasPriceInflationNumerator,
    PessimisticGasPriceInflationDenominator,

    // Account creation config
    MinAllowedTopLevelAccountLength,
    RegistrarAccountId,

    // Storage usage config
    StorageAmountPerByte,
    StorageNumBytesAccount,
    StorageNumExtraBytesRecord,

    // Static action costs
    // send_sir / send_not_sir is burned when creating a receipt on the signer shard.
    // (SIR = signer is receiver, which guarantees the receipt is local.)
    // Execution is burned when applying receipt on receiver shard.
    ActionReceiptCreationSendSir,
    ActionReceiptCreationSendNotSir,
    ActionReceiptCreationExecution,
    DataReceiptCreationBaseSendSir,
    DataReceiptCreationBaseSendNotSir,
    DataReceiptCreationBaseExecution,
    DataReceiptCreationPerByteSendSir,
    DataReceiptCreationPerByteSendNotSir,
    DataReceiptCreationPerByteExecution,
    ActionCreateAccountSendSir,
    ActionCreateAccountSendNotSir,
    ActionCreateAccountExecution,
    ActionDeleteAccountSendSir,
    ActionDeleteAccountSendNotSir,
    ActionDeleteAccountExecution,
    ActionDeployContractSendSir,
    ActionDeployContractSendNotSir,
    ActionDeployContractExecution,
    ActionDeployContractPerByteSendSir,
    ActionDeployContractPerByteSendNotSir,
    ActionDeployContractPerByteExecution,
    ActionFunctionCallSendSir,
    ActionFunctionCallSendNotSir,
    ActionFunctionCallExecution,
    ActionFunctionCallPerByteSendSir,
    ActionFunctionCallPerByteSendNotSir,
    ActionFunctionCallPerByteExecution,
    ActionTransferSendSir,
    ActionTransferSendNotSir,
    ActionTransferExecution,
    ActionStakeSendSir,
    ActionStakeSendNotSir,
    ActionStakeExecution,
    ActionAddFullAccessKeySendSir,
    ActionAddFullAccessKeySendNotSir,
    ActionAddFullAccessKeyExecution,
    ActionAddFunctionCallKeySendSir,
    ActionAddFunctionCallKeySendNotSir,
    ActionAddFunctionCallKeyExecution,
    ActionAddFunctionCallKeyPerByteSendSir,
    ActionAddFunctionCallKeyPerByteSendNotSir,
    ActionAddFunctionCallKeyPerByteExecution,
    ActionDeleteKeySendSir,
    ActionDeleteKeySendNotSir,
    ActionDeleteKeyExecution,

    // Smart contract dynamic gas costs
    WasmRegularOpCost,
    WasmGrowMemCost,
    /// Base cost for a host function
    WasmBase,
    WasmContractLoadingBase,
    WasmContractLoadingBytes,
    WasmReadMemoryBase,
    WasmReadMemoryByte,
    WasmWriteMemoryBase,
    WasmWriteMemoryByte,
    WasmReadRegisterBase,
    WasmReadRegisterByte,
    WasmWriteRegisterBase,
    WasmWriteRegisterByte,
    WasmUtf8DecodingBase,
    WasmUtf8DecodingByte,
    WasmUtf16DecodingBase,
    WasmUtf16DecodingByte,
    WasmSha256Base,
    WasmSha256Byte,
    WasmKeccak256Base,
    WasmKeccak256Byte,
    WasmKeccak512Base,
    WasmKeccak512Byte,
    WasmRipemd160Base,
    WasmRipemd160Block,
    WasmEcrecoverBase,
    WasmEd25519VerifyBase,
    WasmEd25519VerifyByte,
    WasmBls12381VerifyBase,
    WasmBls12381VerifyByte,
    WasmBls12381VerifyElements,
    WasmLogBase,
    WasmLogByte,
    WasmStorageWriteBase,
    WasmStorageWriteKeyByte,
    WasmStorageWriteValueByte,
    WasmStorageWriteEvictedByte,
    WasmStorageReadBase,
    WasmStorageReadKeyByte,
    WasmStorageReadValueByte,
    WasmStorageRemoveBase,
    WasmStorageRemoveKeyByte,
    WasmStorageRemoveRetValueByte,
    WasmStorageHasKeyBase,
    WasmStorageHasKeyByte,
    WasmStorageIterCreatePrefixBase,
    WasmStorageIterCreatePrefixByte,
    WasmStorageIterCreateRangeBase,
    WasmStorageIterCreateFromByte,
    WasmStorageIterCreateToByte,
    WasmStorageIterNextBase,
    WasmStorageIterNextKeyByte,
    WasmStorageIterNextValueByte,
    WasmTouchingTrieNode,
    WasmReadCachedTrieNode,
    WasmPromiseAndBase,
    WasmPromiseAndPerPromise,
    WasmPromiseReturn,
    WasmValidatorStakeBase,
    WasmValidatorTotalStakeBase,
    WasmAltBn128G1MultiexpBase,
    WasmAltBn128G1MultiexpElement,
    WasmAltBn128PairingCheckBase,
    WasmAltBn128PairingCheckElement,
    WasmAltBn128G1SumBase,
    WasmAltBn128G1SumElement,

    // Smart contract limits
    MaxGasBurnt,
    MaxGasBurntView,
    MaxStackHeight,
    StackLimiterVersion,
    InitialMemoryPages,
    MaxMemoryPages,
    RegistersMemoryLimit,
    MaxRegisterSize,
    MaxNumberRegisters,
    MaxNumberLogs,
    MaxTotalLogLength,
    MaxTotalPrepaidGas,
    MaxActionsPerReceipt,
    MaxNumberBytesMethodNames,
    MaxLengthMethodName,
    MaxArgumentsLength,
    MaxLengthReturnedData,
    MaxContractSize,
    MaxTransactionSize,
    MaxLengthStorageKey,
    MaxLengthStorageValue,
    MaxPromisesPerFunctionCallAction,
    MaxNumberInputDataDependencies,
    MaxFunctionsNumberPerContract,
    Wasmer2StackLimit,
    MaxLocalsPerContract,
    AccountIdValidityRulesVersion,
}

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::IntoStaticStr,
    strum::EnumString,
    Debug,
    strum::Display,
)]
#[strum(serialize_all = "snake_case")]
pub enum FeeParameter {
    ActionReceiptCreation,
    DataReceiptCreationBase,
    DataReceiptCreationPerByte,
    ActionCreateAccount,
    ActionDeleteAccount,
    ActionDeployContract,
    ActionDeployContractPerByte,
    ActionFunctionCall,
    ActionFunctionCallPerByte,
    ActionTransfer,
    ActionStake,
    ActionAddFullAccessKey,
    ActionAddFunctionCallKey,
    ActionAddFunctionCallKeyPerByte,
    ActionDeleteKey,
}

impl Parameter {
    /// Iterate through all parameters that define external gas costs that may
    /// be charged during WASM execution. These are essentially all costs from
    /// host function calls. Note that the gas cost for regular WASM operation
    /// is treated separately and therefore not included in this list.
    pub fn ext_costs() -> slice::Iter<'static, Parameter> {
        [
            Parameter::WasmBase,
            Parameter::WasmContractLoadingBase,
            Parameter::WasmContractLoadingBytes,
            Parameter::WasmReadMemoryBase,
            Parameter::WasmReadMemoryByte,
            Parameter::WasmWriteMemoryBase,
            Parameter::WasmWriteMemoryByte,
            Parameter::WasmReadRegisterBase,
            Parameter::WasmReadRegisterByte,
            Parameter::WasmWriteRegisterBase,
            Parameter::WasmWriteRegisterByte,
            Parameter::WasmUtf8DecodingBase,
            Parameter::WasmUtf8DecodingByte,
            Parameter::WasmUtf16DecodingBase,
            Parameter::WasmUtf16DecodingByte,
            Parameter::WasmSha256Base,
            Parameter::WasmSha256Byte,
            Parameter::WasmKeccak256Base,
            Parameter::WasmKeccak256Byte,
            Parameter::WasmKeccak512Base,
            Parameter::WasmKeccak512Byte,
            Parameter::WasmRipemd160Base,
            Parameter::WasmRipemd160Block,
            Parameter::WasmEcrecoverBase,
            Parameter::WasmEd25519VerifyBase,
            Parameter::WasmEd25519VerifyByte,
            Parameter::WasmBls12381VerifyBase,
            Parameter::WasmBls12381VerifyByte,
            Parameter::WasmBls12381VerifyElements,
            Parameter::WasmLogBase,
            Parameter::WasmLogByte,
            Parameter::WasmStorageWriteBase,
            Parameter::WasmStorageWriteKeyByte,
            Parameter::WasmStorageWriteValueByte,
            Parameter::WasmStorageWriteEvictedByte,
            Parameter::WasmStorageReadBase,
            Parameter::WasmStorageReadKeyByte,
            Parameter::WasmStorageReadValueByte,
            Parameter::WasmStorageRemoveBase,
            Parameter::WasmStorageRemoveKeyByte,
            Parameter::WasmStorageRemoveRetValueByte,
            Parameter::WasmStorageHasKeyBase,
            Parameter::WasmStorageHasKeyByte,
            Parameter::WasmStorageIterCreatePrefixBase,
            Parameter::WasmStorageIterCreatePrefixByte,
            Parameter::WasmStorageIterCreateRangeBase,
            Parameter::WasmStorageIterCreateFromByte,
            Parameter::WasmStorageIterCreateToByte,
            Parameter::WasmStorageIterNextBase,
            Parameter::WasmStorageIterNextKeyByte,
            Parameter::WasmStorageIterNextValueByte,
            Parameter::WasmTouchingTrieNode,
            Parameter::WasmReadCachedTrieNode,
            Parameter::WasmPromiseAndBase,
            Parameter::WasmPromiseAndPerPromise,
            Parameter::WasmPromiseReturn,
            Parameter::WasmValidatorStakeBase,
            Parameter::WasmValidatorTotalStakeBase,
            Parameter::WasmAltBn128G1MultiexpBase,
            Parameter::WasmAltBn128G1MultiexpElement,
            Parameter::WasmAltBn128PairingCheckBase,
            Parameter::WasmAltBn128PairingCheckElement,
            Parameter::WasmAltBn128G1SumBase,
            Parameter::WasmAltBn128G1SumElement,
        ]
        .iter()
    }

    /// Iterate through all parameters that define numerical limits for
    /// contracts that are executed in the WASM VM.
    pub fn vm_limits() -> slice::Iter<'static, Parameter> {
        [
            Parameter::MaxGasBurnt,
            Parameter::MaxStackHeight,
            Parameter::StackLimiterVersion,
            Parameter::InitialMemoryPages,
            Parameter::MaxMemoryPages,
            Parameter::RegistersMemoryLimit,
            Parameter::MaxRegisterSize,
            Parameter::MaxNumberRegisters,
            Parameter::MaxNumberLogs,
            Parameter::MaxTotalLogLength,
            Parameter::MaxTotalPrepaidGas,
            Parameter::MaxActionsPerReceipt,
            Parameter::MaxNumberBytesMethodNames,
            Parameter::MaxLengthMethodName,
            Parameter::MaxArgumentsLength,
            Parameter::MaxLengthReturnedData,
            Parameter::MaxContractSize,
            Parameter::MaxTransactionSize,
            Parameter::MaxLengthStorageKey,
            Parameter::MaxLengthStorageValue,
            Parameter::MaxPromisesPerFunctionCallAction,
            Parameter::MaxNumberInputDataDependencies,
            Parameter::MaxFunctionsNumberPerContract,
            Parameter::Wasmer2StackLimit,
            Parameter::MaxLocalsPerContract,
            Parameter::AccountIdValidityRulesVersion,
        ]
        .iter()
    }
}
