#!/usr/bin/env python3
"""Check the working wire contracts, retaining the archived compatibility cases."""
import argparse
import importlib.util
import json
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from openapi_spec_validator import validate
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
HISTORY = {'list_history', 'save_history', 'remove_history', 'clear_history'}
CACHE = {'get_media_cache', 'clear_media_cache'}


def check(directory):
    module = importlib.util.spec_from_file_location('archived_contracts', ROOT / 'replica-spec/validate_contracts.py')
    original = importlib.util.module_from_spec(module)
    module.loader.exec_module(original)
    read = lambda name: json.loads((directory / name).read_text(encoding='utf-8'))
    bridge, app, fixture = (read(name) for name in ('bridge.openapi.json', 'app.schema.json', 'examples/contract-cases.json'))
    baseline_app = original.read('app.schema.json')
    baseline_fixture = original.read('examples/contract-cases.json')
    # Only these optional TV2 fields may differ from the archived bridge.
    nonempty = {'type': 'string', 'minLength': 1, 'maxLength': 128, 'pattern': r'\S'}
    option = {'type': 'object', 'properties': {'value': nonempty, 'name': nonempty},
              'required': ['value', 'name'], 'additionalProperties': False}
    facet = {'type': 'object', 'properties': {'available': {'type': 'boolean'},
             'options': {'type': 'array', 'items': option}}, 'required': ['available', 'options'], 'additionalProperties': False}
    dimensions = ['genre', 'year', 'language', 'region', 'sort']
    count = {'type': 'integer', 'minimum': 0, 'maximum': 9007199254740991}
    allowed = {
        'MediaCard': {
            'backdrop': {'anyOf': [{'$ref': '#/components/schemas/ImageRef'}, {'type': 'null'}]},
            'plot': {'anyOf': [{'type': 'string', 'minLength': 1, 'pattern': r'\S'}, {'type': 'null'}]},
            'genres': {'anyOf': [{'type': 'array', 'minItems': 1, 'items': nonempty}, {'type': 'null'}]},
        },
        'HomeSection': {
            'kind': {'type': 'string', 'enum': ['movie', 'series', 'anime', 'other']},
            'filters': {'type': 'object', 'properties': {key: facet for key in dimensions},
                        'required': dimensions, 'additionalProperties': False},
            'scanned_count': count, 'matched_count': count,
        },
        'HomeRequest': {'filter': {'anyOf': [
            {'type': 'object', 'properties': {**{key: nonempty for key in dimensions if key != 'sort'},
                'sort': {'type': 'string', 'enum': ['trending_day', 'trending_week', 'popular', 'latest']}},
                'additionalProperties': False}, {'type': 'null'}]}},
    }
    archived_bridge = json.loads(json.dumps(bridge))
    for name, fields in allowed.items():
        actual = archived_bridge['components']['schemas'][name]
        for field, definition in fields.items():
            assert field not in actual['required'], f'TV2 field must remain optional: {name}.{field}'
            assert actual['properties'].pop(field, None) == definition, f'TV2 field changed: {name}.{field}'
    assert archived_bridge == original.read('bridge.openapi.json'), 'Existing bridge field or definition changed'
    for name, definition in baseline_app['$defs'].items():
        actual = json.loads(json.dumps(app['$defs'][name]))
        if name == 'Settings':
            assert 'metadata_preset_applied' not in actual['required'], 'Preset marker must remain optional'
            assert actual['properties'].pop('metadata_preset_applied', None) == {'type': 'boolean'}, 'Preset marker contract changed'
            assert actual['properties'].pop('media_cache_max_bytes', None) == {'type': 'integer', 'minimum': 0, 'maximum': 9007199254740991, 'default': 268435456}, 'Cache setting contract changed'
        assert actual == definition, f'Existing DTO changed: {name}'
    for key in ('x-command-contracts', 'x-event-contracts', 'x-request-budgets', 'x-stremio-capability-rule'):
        for name, definition in baseline_app[key].items():
            assert app[key][name] == definition, f'Existing {key} changed: {name}'
    commands = app['x-command-contracts']
    additions = set(commands) - set(baseline_app['x-command-contracts'])
    assert additions == HISTORY | CACHE, 'History and media cache commands must have their complete contracts'
    if (ROOT / 'src-tauri/src/history.rs').exists():
        assert HISTORY <= additions, 'Native history requires all four IPC contracts'
    for command in CACHE:
        assert app["$defs"]["Args_" + command]["properties"]["input"] == {"type": "object", "properties": {}, "additionalProperties": False}, "Cache commands accept no path or sequencing input"
    validate(bridge)
    Draft202012Validator.check_schema(app)
    registry = Registry().with_resource(original.BRIDGE_URI, Resource.from_contents({
        '$schema': 'https://json-schema.org/draft/2020-12/schema', '$id': original.BRIDGE_URI, **bridge,
    })).with_resource(app['$id'], Resource.from_contents(app))
    documents = {'bridge.openapi.json': bridge, 'app.schema.json': app}
    def refs(value, owner):
        if isinstance(value, dict):
            if '$ref' in value:
                target, pointer = value['$ref'].split('#', 1)
                selected = documents[target or owner]
                for part in pointer.lstrip('/').split('/'):
                    selected = selected[part.replace('~1', '/').replace('~0', '~')]
            for child in value.values():
                refs(child, owner)
        elif isinstance(value, list):
            for child in value:
                refs(child, owner)
    for name, document in documents.items():
        refs(document, name)
    for key in ('cases', 'semantic_cases', 'lifecycle_vectors'):
        for case in baseline_fixture[key]:
            assert case in fixture[key], f'Archived regression case changed: {key}'
    cases = fixture['cases']
    assert cases and len({case['name'] for case in cases}) == len(cases), 'Missing or duplicate cases'
    for case in cases:
        pointer = (f'{original.BRIDGE_URI}#/components/schemas/{case["schema"]}' if case['document'] == 'bridge'
                   else f'{app["$id"]}#/$defs/{case["schema"]}')
        validator = Draft202012Validator({'$ref': pointer}, registry=registry, format_checker=FormatChecker())
        assert (not list(validator.iter_errors(case['value']))) == case['expected_valid'], case['name']
        if case['expected_valid']:
            for fn in (original.check_source_bindings, original.check_playback_resolutions,
                       original.check_cancel_identities, original.check_home_positions):
                fn(case['value'])
            if case['schema'] == 'WatchHistoryRecord':
                assert original.history_record(case['value']), case['name']
    names = {case['name'] for case in cases}
    for command in commands:
        assert commands[command] == {'args': {'$ref': '#/$defs/Args_' + command}, 'result': {'$ref': '#/$defs/Result_' + command}}, command
        assert {'args_' + command, 'result_' + command} <= names, f'Missing command cases: {command}'
    for command in additions:
        for prefix in ('Args_', 'Result_'):
            expected = {case['expected_valid'] for case in cases if case['schema'] == prefix + command}
            assert expected == {True, False}, f'History needs positive and negative cases: {prefix}{command}'
    for case in fixture['semantic_cases']:
        assert original.semantic(case) == case['expected_valid'], case['name']
    for case in fixture['lifecycle_vectors']:
        assert original.lifecycle(case), case['name']
    return {'result': 'PASS', 'commands': len(commands), 'structural_cases': len(cases),
            'semantic_cases': len(fixture['semantic_cases']), 'lifecycle_vectors': len(fixture['lifecycle_vectors'])}


if __name__ == '__main__':
    if not __debug__:
        raise SystemExit('Contract checks require assertions; remove -O')
    parser = argparse.ArgumentParser()
    parser.add_argument('--protocol-dir', type=Path, default=ROOT / 'protocol')
    options = parser.parse_args()
    print(json.dumps(check(options.protocol_dir), ensure_ascii=False))
