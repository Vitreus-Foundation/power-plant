from substrateinterface import SubstrateInterface, Keypair, KeypairType
from eth_abi import encode
from eth_utils import keccak, to_bytes, to_checksum_address
from substrateinterface.base import GenericCall, ScaleBytes, SubstrateRequestException
from scalecodec import U256


def func_selector(text):
    return keccak(text=text)[:4]


def encode_input(name, types, args):
    function_signature = func_selector(name)
    return function_signature + encode(types, args)


def num_to_address(num):
    address_hex = hex(num)[2:]  # Remove the '0x' prefix

    address_hex_padded = address_hex.zfill(40)

    return to_checksum_address(address_hex_padded)


def init_interface():
    substrate = SubstrateInterface(url="ws://127.0.0.1:9944", use_remote_preset=True)
    substrate.init_runtime()
    substrate.runtime_config.update_type_registry(
        {
            # "types": {
            #     "Address": "H160",
            #     "LookupSource": "H160",
            #     "AccountId": "H160",
            #     "ExtrinsicSignature": "EcdsaSignature",
            # }
            "types": {
                "Address": "AccountId",
                "LookupSource": "AccountId",
                "Account": {
                    "type": "struct",
                    "type_mapping": [["nonce", "U256"], ["balance", "U256"]],
                },
                "AccountId": "H160",
                "Transaction": {
                    "type": "struct",
                    "type_mapping": [
                        ["nonce", "U256"],
                        ["gas_price", "U256"],
                        ["gas_limit", "U256"],
                        ["action", "EthTransactionAction"],
                        ["value", "U256"],
                        ["input", "Bytes"],
                        ["signature", "EthTransactionSignature"],
                    ],
                },
                "Signature": {
                    "type": "struct",
                    "type_mapping": [["v", "u64"], ["r", "H256"], ["s", "H256"]],
                },
            }
        }
    )
    return substrate


def call_contract(interface, keypair, contract_addr, input):
    # obj = interface.runtime_config.create_scale_object("u256")
    # print(obj.encode(10000))
    # print(U256().encode(10000))
    # print(interface.encode_scale("u256", 10000))
    # print(
    #     U256(
    #         ScaleBytes(
    #             "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    #         )
    #     ).decode()
    # )
    # print(
    #         U256(1000).encode()
    # )
    # list(bytearray.fromhex(
    #     "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    # ))
    meta = interface.get_metadata()
    call = interface.runtime_config.create_scale_object("Call", metadata=meta)
    cc = call.encode(
        {
            "call_module": "EVM",
            "call_function": "call",
            "call_args": {
                "source": "0x" + keypair.public_key.hex(),
                "target": contract_addr,
                "input": input.hex(),
                "value": 0,
                "gas_limit": 10000000,
                "max_fee_per_gas": 10000000,
                "max_priority_fee_per_gas": None,
                "nonce": None,
                "access_list": [],
            },
        },
    )
    print(cc)

    # call = interface.compose_call(
    #     call_module="EVM",
    #     call_function="call",
    #     call_params={
    #         "source": "0x" + keypair.public_key.hex(),
    #         "target": contract_addr,
    #         "input": input.hex(),
    #         "value": 0,
    #         "gas_limit": 10000000,
    #         # "max_fee_per_gas": [
    #         #     list(
    #         #         bytearray.fromhex(
    #         #             "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    #         #         )
    #         #     )
    #         # ],
    #         # "max_fee_per_gas": interface.encode_scale("U256", 10000),
    #         "max_fee_per_gas": 10000000,
    #         "max_priority_fee_per_gas": (None, 0),
    #         "nonce": None,
    #         "access_list": [],
    #     },
    # )
    #
    # extrinsic = interface.create_signed_extrinsic(call=call, keypair=keypair)
    # receipt = interface.submit_extrinsic(extrinsic, wait_for_inclusion=True)
    # print(
    #     f"Extrinsic '{receipt.extrinsic_hash}' sent and included in block '{receipt.block_hash}'"
    # )


CONTRACT_ADDRESS = num_to_address(2050)

interface = init_interface()

alith_keypair = Keypair.create_from_private_key(
    "0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133",
    crypto_type=KeypairType.ECDSA,
)

input = encode_input("name()", [], [])
# print(input)
# type_id = interface.get_metadata_call_function("EVM", "call")
# print(type_id)
call_contract(interface, alith_keypair, CONTRACT_ADDRESS, input)

# extrinsic = substrate.create_signed_extrinsic(call=call, keypair=alith_keypair)
# receipt = substrate.submit_extrinsic(extrinsic, wait_for_inclusion=True)
# print(
#     f"Extrinsic '{receipt.extrinsic_hash}' sent and included in block '{receipt.block_hash}'"
# )
