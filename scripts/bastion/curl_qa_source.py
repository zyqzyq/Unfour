"""Read the production no-network curl probe without duplicating its command.

This deliberately supports the current Rust string/concat!/format! shapes only.
An ownership or syntax change must update the harness rather than use stale text.
"""
import json
import re


def rust_string_constant(source, name):
    literal = r'"(?:[^"\\]|\\.)*"'
    match = re.search(r'\bconst\s+' + re.escape(name) +
                      r':\s*&str\s*=\s*(' + literal + r'|concat!\(.*?\n\))\s*;',
                      source, re.S)
    if not match:
        raise ValueError(f'Rust string constant not found: {name}')
    value = match.group(1)
    if value.startswith('"'):
        return json.loads(value)
    return ''.join(json.loads(item) for item in re.findall(literal, value))


def curl_probe_command(root):
    contract = (root / 'crates/unfour-core/src/terminal_curl_contract.rs').read_text(encoding='utf-8')
    return (rust_string_constant(contract, 'CURL_SETUP') +
            rust_string_constant(contract, 'HTTP_VERSION_CHECK') +
            'command printf UNFOUR_CURL_TOOLS; exit 0')
