#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
NETWORK_CONFIG="${BLNT_FILLER_NETWORK_CONFIG:-${ROOT_DIR}/config/mainnet.json}"
RELEASE_MANIFEST="${BLNT_FILLER_RELEASE_MANIFEST:-${ROOT_DIR}/releases/v0.1.0.json}"
WASM="${ROOT_DIR}/target/wasm32v1-none/optimized/blnt_interest_auction_filler.wasm"
COMMAND="${1:-plan}"

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

for tool in jq make stellar awk; do
    require_command "${tool}"
done

[[ -f "${NETWORK_CONFIG}" ]] || die "network configuration not found: ${NETWORK_CONFIG}"
[[ -f "${RELEASE_MANIFEST}" ]] || die "release manifest not found: ${RELEASE_MANIFEST}"

NETWORK_NAME="$(jq -er '.network.name' "${NETWORK_CONFIG}")"
NETWORK_PASSPHRASE="$(jq -er '.network.passphrase' "${NETWORK_CONFIG}")"
RPC_URL="${BLNT_FILLER_RPC_URL:-$(jq -er '.network.default_rpc_url' "${NETWORK_CONFIG}")}"
EXPECTED_HASH="$(jq -er '.artifact.sha256' "${RELEASE_MANIFEST}")"
EXPECTED_SIZE="$(jq -er '.artifact.bytes' "${RELEASE_MANIFEST}")"
RELEASE_VERSION="$(jq -er '.version' "${RELEASE_MANIFEST}")"
SOURCE_REPOSITORY="$(jq -er '.source_repository' "${RELEASE_MANIFEST}")"
SOURCE_COMMIT="$(jq -er '.source_commit' "${RELEASE_MANIFEST}")"
BACKSTOP="$(jq -er '.constructor_bindings.backstop' "${NETWORK_CONFIG}")"
BLNT="$(jq -er '.constructor_bindings.blnt' "${NETWORK_CONFIG}")"
USDC="$(jq -er '.constructor_bindings.usdc' "${NETWORK_CONFIG}")"
XLM="$(jq -er '.constructor_bindings.xlm' "${NETWORK_CONFIG}")"
BLNT_USDC="$(jq -er '.constructor_bindings.blnt_usdc' "${NETWORK_CONFIG}")"
BLNT_XLM="$(jq -er '.constructor_bindings.blnt_xlm' "${NETWORK_CONFIG}")"
SOURCE_ACCOUNT="${BLNT_FILLER_SOURCE_ACCOUNT:-}"
OWNER="${BLNT_FILLER_OWNER:-}"
INCLUSION_FEE="${BLNT_FILLER_INCLUSION_FEE:-}"

NETWORK_ARGS=(
    --rpc-url "${RPC_URL}"
    --network-passphrase "${NETWORK_PASSPHRASE}"
)
FEE_ARGS=()
if [[ -n "${INCLUSION_FEE}" ]]; then
    [[ "${INCLUSION_FEE}" =~ ^[0-9]+$ ]] || die "BLNT_FILLER_INCLUSION_FEE must be an integer number of stroops"
    FEE_ARGS=(--inclusion-fee "${INCLUSION_FEE}")
fi

verify_release() {
    local actual_hash actual_size expected_metadata actual_version
    make -C "${ROOT_DIR}" build
    actual_hash="$(sha256_file "${WASM}")"
    actual_size="$(wc -c < "${WASM}" | tr -d '[:space:]')"
    [[ "${actual_hash}" == "${EXPECTED_HASH}" ]] ||
        die "WASM hash ${actual_hash} does not match release ${EXPECTED_HASH}"
    [[ "${actual_size}" == "${EXPECTED_SIZE}" ]] ||
        die "WASM size ${actual_size} does not match release ${EXPECTED_SIZE}"
    expected_metadata="$(jq -er '.artifact.source_repo_metadata' "${RELEASE_MANIFEST}")"
    stellar contract info meta --wasm "${WASM}" 2>/dev/null |
        grep -Fq "source_repo: ${expected_metadata}" ||
        die "WASM source_repo metadata does not match the release manifest"
    actual_version="$(stellar --version | awk 'NR == 1 {print $2}')"
    [[ "${actual_version}" == "$(jq -er '.toolchain.stellar_cli' "${RELEASE_MANIFEST}")" ]] ||
        die "Stellar CLI ${actual_version} does not match the pinned release toolchain"
    printf 'Verified release %s: %s (%s bytes)\n' \
        "${RELEASE_VERSION}" "${EXPECTED_HASH}" "${EXPECTED_SIZE}"
}

print_plan() {
    verify_release
    printf '%s\n' \
        "Network: ${NETWORK_NAME}" \
        "RPC: ${RPC_URL}" \
        "Owner: ${OWNER:-<set BLNT_FILLER_OWNER>}" \
        "Backstop: ${BACKSTOP}" \
        "BLNT: ${BLNT}" \
        "USDC: ${USDC}" \
        "XLM: ${XLM}" \
        "BLNT:USDC: ${BLNT_USDC}" \
        "BLNT:XLM: ${BLNT_XLM}"
}

require_owner() {
    [[ "${OWNER}" =~ ^G[A-Z2-7]{55}$ ]] ||
        die "BLNT_FILLER_OWNER must be the filler account's public G address"
}

require_source() {
    [[ -n "${SOURCE_ACCOUNT}" ]] ||
        die "BLNT_FILLER_SOURCE_ACCOUNT must name a funded Stellar CLI identity"
    [[ ! "${SOURCE_ACCOUNT}" =~ ^S[A-Z2-7]{55}$ ]] ||
        die "use a Stellar CLI identity instead of placing a secret key in BLNT_FILLER_SOURCE_ACCOUNT"
}

TMP_DIR=""
cleanup() {
    if [[ -n "${TMP_DIR}" && -d "${TMP_DIR}" ]]; then
        rm -f "${TMP_DIR}/stdout" "${TMP_DIR}/stderr" "${TMP_DIR}/network.wasm" \
            "${TMP_DIR}/contract.wasm" "${TMP_DIR}/fetch.stderr" \
            "${TMP_DIR}/record.json"
        rmdir "${TMP_DIR}" 2>/dev/null || true
    fi
}
trap cleanup EXIT

ensure_tmp_dir() {
    if [[ -z "${TMP_DIR}" ]]; then
        TMP_DIR="$(mktemp -d)"
    fi
}

LAST_OUTPUT=""
LAST_TX_HASH=""
run_transaction() {
    ensure_tmp_dir
    : > "${TMP_DIR}/stdout"
    : > "${TMP_DIR}/stderr"
    if ! "$@" > "${TMP_DIR}/stdout" 2> >(tee "${TMP_DIR}/stderr" >&2); then
        cat "${TMP_DIR}/stdout" >&2
        return 1
    fi
    LAST_OUTPUT="$(cat "${TMP_DIR}/stdout")"
    if [[ -n "${LAST_OUTPUT}" ]]; then
        printf '%s\n' "${LAST_OUTPUT}"
    fi
    LAST_TX_HASH="$(
        grep -E 'Signing transaction:' "${TMP_DIR}/stderr" |
            grep -Eo '[0-9a-f]{64}' |
            tail -n 1 || true
    )"
}

fetch_installed_wasm() {
    ensure_tmp_dir
    rm -f "${TMP_DIR}/network.wasm" "${TMP_DIR}/fetch.stderr"
    if ! stellar contract fetch "${NETWORK_ARGS[@]}" --wasm-hash "${EXPECTED_HASH}" \
        --out-file "${TMP_DIR}/network.wasm" >/dev/null 2> "${TMP_DIR}/fetch.stderr"; then
        if grep -Fq 'Contract Code not found' "${TMP_DIR}/fetch.stderr"; then
            return 1
        fi
        cat "${TMP_DIR}/fetch.stderr" >&2
        die "could not determine whether the released WASM is installed"
    fi
    [[ "$(sha256_file "${TMP_DIR}/network.wasm")" == "${EXPECTED_HASH}" ]] ||
        die "network returned unexpected WASM for ${EXPECTED_HASH}"
}

UPLOAD_TX_HASH=""
ensure_wasm_installed() {
    if fetch_installed_wasm; then
        printf 'WASM is already installed on %s.\n' "${NETWORK_NAME}"
        return
    fi
    printf 'Uploading released WASM to %s...\n' "${NETWORK_NAME}"
    run_transaction stellar contract upload "${NETWORK_ARGS[@]}" "${FEE_ARGS[@]}" \
        --source-account "${SOURCE_ACCOUNT}" --wasm "${WASM}" --optimize=false
    local uploaded_hash
    uploaded_hash="$(printf '%s' "${LAST_OUTPUT}" | grep -Eo '[0-9a-f]{64}' | tail -n 1)"
    [[ "${uploaded_hash}" == "${EXPECTED_HASH}" ]] ||
        die "uploaded WASM hash ${uploaded_hash:-<missing>} does not match ${EXPECTED_HASH}"
    UPLOAD_TX_HASH="${LAST_TX_HASH}"
    fetch_installed_wasm || die "uploaded WASM cannot be fetched from the network"
}

verify_instance() {
    local contract_id="$1" expected_owner="$2" view_source actual_hash actual_owner
    [[ "${contract_id}" =~ ^C[A-Z2-7]{55}$ ]] || die "invalid contract ID: ${contract_id}"
    [[ "${expected_owner}" =~ ^G[A-Z2-7]{55}$ ]] || die "invalid expected owner: ${expected_owner}"
    ensure_tmp_dir
    rm -f "${TMP_DIR}/contract.wasm"
    stellar contract fetch "${NETWORK_ARGS[@]}" --id "${contract_id}" \
        --out-file "${TMP_DIR}/contract.wasm" >/dev/null
    actual_hash="$(sha256_file "${TMP_DIR}/contract.wasm")"
    [[ "${actual_hash}" == "${EXPECTED_HASH}" ]] ||
        die "contract uses WASM ${actual_hash}, expected ${EXPECTED_HASH}"
    view_source="${SOURCE_ACCOUNT:-${expected_owner}}"
    actual_owner="$(
        stellar contract invoke --send no "${NETWORK_ARGS[@]}" \
            --source-account "${view_source}" --id "${contract_id}" -- get_owner |
            tr -d '"[:space:]'
    )"
    [[ "${actual_owner}" == "${expected_owner}" ]] ||
        die "contract owner ${actual_owner} does not match ${expected_owner}"
    printf 'Verified instance %s with owner %s.\n' "${contract_id}" "${expected_owner}"
}

deploy_instance() {
    local salt deployer contract_id deploy_tx_hash record_path timestamp verified_at
    verify_release
    require_source
    require_owner
    require_command openssl
    deployer="$(stellar keys address "${SOURCE_ACCOUNT}" 2>/dev/null || true)"
    if [[ -z "${deployer}" && "${SOURCE_ACCOUNT}" =~ ^G[A-Z2-7]{55}$ ]]; then
        deployer="${SOURCE_ACCOUNT}"
    fi
    [[ "${deployer}" =~ ^G[A-Z2-7]{55}$ ]] ||
        die "cannot resolve BLNT_FILLER_SOURCE_ACCOUNT to a public address"
    ensure_wasm_installed
    salt="${BLNT_FILLER_SALT:-$(openssl rand -hex 32)}"
    [[ "${salt}" =~ ^[0-9a-fA-F]{64}$ ]] ||
        die "BLNT_FILLER_SALT must contain exactly 64 hexadecimal characters"
    printf 'Deploying owner-specific filler instance with salt %s...\n' "${salt}"
    run_transaction stellar contract deploy "${NETWORK_ARGS[@]}" "${FEE_ARGS[@]}" \
        --source-account "${SOURCE_ACCOUNT}" --wasm-hash "${EXPECTED_HASH}" --salt "${salt}" -- \
        --owner "${OWNER}" \
        --backstop "${BACKSTOP}" \
        --blnt "${BLNT}" \
        --usdc "${USDC}" \
        --xlm "${XLM}" \
        --blnt_usdc "${BLNT_USDC}" \
        --blnt_xlm "${BLNT_XLM}"
    contract_id="$(printf '%s' "${LAST_OUTPUT}" | grep -Eo 'C[A-Z2-7]{55}' | tail -n 1)"
    [[ "${contract_id}" =~ ^C[A-Z2-7]{55}$ ]] ||
        die "deployment succeeded but no contract ID was returned"
    deploy_tx_hash="${LAST_TX_HASH}"
    timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    record_path="${BLNT_FILLER_RECORD:-${ROOT_DIR}/deployments/mainnet-${contract_id}.json}"
    jq -n \
        --arg generated_at "${timestamp}" \
        --arg network_name "${NETWORK_NAME}" \
        --arg network_passphrase "${NETWORK_PASSPHRASE}" \
        --arg rpc_url "${RPC_URL}" \
        --arg release_version "${RELEASE_VERSION}" \
        --arg source_repository "${SOURCE_REPOSITORY}" \
        --arg source_commit "${SOURCE_COMMIT}" \
        --arg wasm_hash "${EXPECTED_HASH}" \
        --argjson wasm_bytes "${EXPECTED_SIZE}" \
        --arg contract_id "${contract_id}" \
        --arg owner "${OWNER}" \
        --arg deployer "${deployer}" \
        --arg salt "${salt}" \
        --arg backstop "${BACKSTOP}" \
        --arg blnt "${BLNT}" \
        --arg usdc "${USDC}" \
        --arg xlm "${XLM}" \
        --arg blnt_usdc "${BLNT_USDC}" \
        --arg blnt_xlm "${BLNT_XLM}" \
        --arg upload_tx "${UPLOAD_TX_HASH}" \
        --arg deploy_tx "${deploy_tx_hash}" \
        '{
          schema_version: 1,
          generated_at: $generated_at,
          network: {
            name: $network_name,
            passphrase: $network_passphrase,
            rpc_url: $rpc_url
          },
          release: {
            version: $release_version,
            source_repository: $source_repository,
            source_commit: $source_commit,
            wasm_sha256: $wasm_hash,
            wasm_bytes: $wasm_bytes
          },
          contract: {
            id: $contract_id,
            owner: $owner,
            deployer: $deployer,
            salt: $salt,
            constructor_bindings: {
              backstop: $backstop,
              blnt: $blnt,
              usdc: $usdc,
              xlm: $xlm,
              blnt_usdc: $blnt_usdc,
              blnt_xlm: $blnt_xlm
            }
          },
          transactions: {
            upload_wasm: (if $upload_tx == "" then null else $upload_tx end),
            deploy_contract: (if $deploy_tx == "" then null else $deploy_tx end)
          },
          verification: {
            verified_at: null,
            wasm_sha256: null,
            owner: null
          }
        }' > "${record_path}"
    chmod 600 "${record_path}"
    printf 'Preliminary operator deployment record: %s\n' "${record_path}"
    verify_instance "${contract_id}" "${OWNER}"
    verified_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    jq \
        --arg verified_at "${verified_at}" \
        --arg wasm_hash "${EXPECTED_HASH}" \
        --arg owner "${OWNER}" \
        '.verification = {
          verified_at: $verified_at,
          wasm_sha256: $wasm_hash,
          owner: $owner
        }' "${record_path}" > "${TMP_DIR}/record.json"
    mv "${TMP_DIR}/record.json" "${record_path}"
    chmod 600 "${record_path}"
    printf 'Verified operator deployment record: %s\n' "${record_path}"
}

case "${COMMAND}" in
    plan)
        print_plan
        ;;
    release-verify)
        verify_release
        ;;
    deploy)
        deploy_instance
        ;;
    verify)
        verify_release
        require_owner
        [[ $# -ge 2 ]] || die "usage: $0 verify C..."
        verify_instance "$2" "${OWNER}"
        ;;
    *)
        die "unknown command '${COMMAND}'; expected plan, release-verify, deploy, or verify"
        ;;
esac
