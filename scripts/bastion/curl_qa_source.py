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
    source = (root / 'crates/ssh-engine/src/ssh/temporary/terminal_operation/probe.rs').read_text(encoding='utf-8')
    literal = r'"(?:[^"\\]|\\.)*"'
    match = re.search(r'fn curl_command\(\)\s*->\s*String\s*\{\s*format!\(\s*(' + literal + ')', source)
    if not match:
        raise ValueError('Production curl_command format string not found')
    template = json.loads(match.group(1))
    contract = (root / 'crates/unfour-core/src/terminal_curl_contract.rs').read_text(encoding='utf-8')
    used = []
    def expand(match):
        name = match.group(1)
        if name not in {'CURL_SETUP'}:
            raise ValueError(f'Unexpected production probe substitution: {name}')
        used.append(name)
        return rust_string_constant(contract, name)
    command = re.sub(r'\{([A-Z][A-Z0-9_]*)\}', expand, template)
    if set(used) != {'CURL_SETUP'}:
        raise ValueError('Production probe contract substitutions changed')
    return command
