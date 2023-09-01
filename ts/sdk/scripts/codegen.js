const codegen = require("@cosmwasm/ts-codegen").default;
const path = require("path");
const fs = require("fs");

const pkgRoot = path.join(__dirname, "..");
const contractsDir = path.join(pkgRoot, "..", "..", "contracts");

// patch definitions
const definitions = {
  TransmuterPool: {
    type: "object",
    required: ["in_coin", "out_coin_reserve"],
    properties: {
      in_coin: {
        description: "incoming coins are stored here",
        allOf: [
          {
            $ref: "#/definitions/Coin",
          },
        ],
      },
      out_coin_reserve: {
        description: "reserve of coins for future transmutations",
        allOf: [
          {
            $ref: "#/definitions/Coin",
          },
        ],
      },
    },
    additionalProperties: false,
  },
  Coin: {
    type: "object",
    required: ["amount", "denom"],
    properties: {
      amount: {
        $ref: "#/definitions/Uint128",
      },
      denom: {
        type: "string",
      },
    },
  },
  Uint128: {
    description:
      "A thin wrapper around u128 that is using strings for JSON encoding/decoding, such that the full u128 range can be used for clients that convert JSON numbers to floats, like JavaScript and jq.\n\n# Examples\n\nUse `from` to create instances of this and `u128` to get the value out:\n\n``` # use cosmwasm_std::Uint128; let a = Uint128::from(123u128); assert_eq!(a.u128(), 123);\n\nlet b = Uint128::from(42u64); assert_eq!(b.u128(), 42);\n\nlet c = Uint128::from(70u32); assert_eq!(c.u128(), 70); ```",
    type: "string",
  },
  Decimal: {
    description:
      "A fixed-point decimal value with 18 fractional digits, i.e. Decimal(1_000_000_000_000_000_000) == 1.0\n\nThe greatest possible value that can be represented is 340282366920938463463.374607431768211455 (which is (2^128 - 1) / 10^18)",
    type: "string",
  },
  Addr: {
    description:
      "A human readable address.\n\nIn Cosmos, this is typically bech32 encoded. But for multi-chain smart contracts no assumptions should be made other than being UTF-8 encoded and of reasonable length.\n\nThis type represents a validated address. It can be created in the following ways 1. Use `Addr::unchecked(input)` 2. Use `let checked: Addr = deps.api.addr_validate(input)?` 3. Use `let checked: Addr = deps.api.addr_humanize(canonical_addr)?` 4. Deserialize from JSON. This must only be done from JSON that was validated before such as a contract's state. `Addr` must not be used in messages sent by the user because this would result in unvalidated instances.\n\nThis type is immutable. If you really need to mutate it (Really? Are you sure?), create a mutable copy using `let mut mutable = Addr::to_string()` and operate on that `String` instance.",
    type: "string",
  },
};

const transmuterSchema = path.join(
  contractsDir,
  "transmuter",
  "schema",
  "transmuter.json"
);

// read transmuter schema as json
const transmuterSchemaJson = JSON.parse(fs.readFileSync(transmuterSchema));
transmuterSchemaJson.definitions = definitions;
fs.writeFileSync(transmuterSchema, JSON.stringify(transmuterSchemaJson));

// codegen

const contracts = fs
  .readdirSync(contractsDir, { withFileTypes: true })
  .filter((c) => c.isDirectory())
  .map((c) => ({
    name: c.name,
    dir: path.join(contractsDir, c.name),
  }));

const outPath = path.join(pkgRoot, "src", "contracts");
fs.rmSync(outPath, { recursive: true, force: true });

codegen({
  contracts,
  outPath,
  options: {
    bundle: {
      bundleFile: "index.ts",
      scope: "contracts",
    },
  },
}).then(() => {
  console.log("✨ Typescript code is generated successfully!");
});
