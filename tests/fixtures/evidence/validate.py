"""Independent, stdlib-only offline evidence-v1 consumer. Never imports Rust DTOs.
Checks the checked-in JSON Schema vocabulary plus cross-record invariants.
"""
import json
import re
import sys
from pathlib import Path

SCHEMA = json.loads((Path(__file__).parents[3] / 'docs/contracts/context-integrity-evidence-v1.schema.json').read_text())
MAX = 8 * 1024 * 1024


def unique(pairs):
    d = {}
    for k, v in pairs:
        if k in d:
            raise ValueError('duplicate key')
        d[k] = v
    return d


def check_schema(v, s):
    if '$ref' in s:
        check_schema(v, SCHEMA['$defs'][s['$ref'].split('/')[-1]])
    if 'const' in s:
        assert type(v) is type(s['const']) and v == s['const']
    if 'enum' in s:
        assert v in s['enum']
    types = {'object': dict, 'array': list, 'string': str, 'integer': int, 'number': (int, float), 'boolean': bool, 'null': type(None)}
    if 'type' in s:
        ts = s['type'] if isinstance(s['type'], list) else [s['type']]
        assert any(type(v) is types[t] if isinstance(types[t], type) else type(v) in types[t] for t in ts)
    for keyword in ['oneOf', 'anyOf']:
        if keyword in s:
            good = 0
            for sub in s[keyword]:
                try:
                    check_schema(v, sub)
                    good += 1
                except (AssertionError, KeyError, TypeError, ValueError):
                    pass
            assert good == 1 if keyword == 'oneOf' else good >= 1
    for sub in s.get('allOf', []):
        check_schema(v, sub)
    if isinstance(v, dict):
        assert all(k in v for k in s.get('required', []))
        props = s.get('properties', {})
        for k, val in v.items():
            if k in props:
                check_schema(val, props[k])
            elif s.get('additionalProperties') is False:
                raise ValueError('unexpected property')
            elif isinstance(s.get('additionalProperties'), dict):
                check_schema(val, s['additionalProperties'])
    if isinstance(v, list):
        for val in v:
            check_schema(val, s.get('items', {}))
    if type(v) in (int, float):
        assert v >= s.get('minimum', v)
        assert v <= s.get('maximum', v)


def safe(p):
    assert isinstance(p, str) and 0 < len(p.encode()) <= 4096
    assert not any(c in p for c in ['\\', ':', '\ufffd'])
    assert not any(ord(c) < 32 or 127 <= ord(c) <= 159 or 0xD800 <= ord(c) <= 0xDFFF for c in p)
    assert all(c not in ['', '.', '..'] and len(c.encode()) <= 255 for c in p.split('/'))


def ref(x):
    assert re.fullmatch('[a-f0-9]{64}', x)


def span(r):
    for p in [r['start'], r['end']]:
        assert all(type(p[k]) is int and p[k] >= 0 for k in ['byte', 'row', 'column'])
    assert r['start']['byte'] <= r['end']['byte']
    assert r['start']['row'] <= r['end']['row']
    if r['start']['row'] == r['end']['row']:
        assert r['start']['column'] <= r['end']['column']


STATUS = {'analyzed', 'unsupported_language', 'skipped_role', 'too_large', 'invalid_utf8', 'read_failed', 'parse_failed', 'analysis_limit_exceeded'}
RELSTATUS = {'analyzed', 'extraction_failed', 'analysis_limit_exceeded'}
KINDS = {'module', 'class', 'struct', 'enum', 'union', 'trait', 'function', 'method', 'type_alias', 'constant', 'static'}
DERIVATION = {'repository_contains_file', 'lexical_range_containment', 'rust_use_syntax', 'python_import_syntax', 'rust_call_syntax', 'python_call_syntax'}
RESOLUTION = {'direct_structural', 'exact_same_file_qualified_name', 'unresolved'}


def file(f):
    ref(f['id']); safe(f['path'])
    assert f['analysis_status'] in STATUS
    assert f['relation_status'] is None or f['relation_status'] in RELSTATUS
    assert type(f['syntax_errors']) is int and f['syntax_errors'] >= 0


def symbol(s, files):
    ref(s['id']); safe(s['path'])
    assert files[s['file_ref']]['path'] == s['path']
    assert s['kind'] in KINDS and s['derivation'] == 'parser_definition'
    span(s['range']); span(s['name_range'])
    assert s['range']['start']['byte'] <= s['name_range']['start']['byte'] <= s['name_range']['end']['byte'] <= s['range']['end']['byte']


def structure(d, coverage):
    ref(d['repository_ref'])
    entities = {d['repository_ref']: ('repository', None)}
    ids = {d['repository_ref']}
    files = {}
    paths = set()
    for f in d['files']:
        file(f)
        assert f['id'] not in ids and f['path'] not in paths
        ids.add(f['id']); paths.add(f['path']); files[f['id']] = f
        entities[f['id']] = ('file', f['path'])
    for s in d['symbols']:
        symbol(s, files)
        assert s['id'] not in ids
        ids.add(s['id'])
        entities[s['id']] = ('symbol', s['path'], s)
    entity_ids = set(ids)
    for r in d['relations']:
        ref(r['id']); safe(r['path'])
        assert r['id'] not in ids and r['source_ref'] in entity_ids
        ids.add(r['id'])
        assert r['resolution'] in RESOLUTION and r['derivation'] in DERIVATION
        assert r['kind'] in {'contains', 'imports', 'references'}
        if r['resolution'] == 'unresolved':
            assert r['target_ref'] is None and isinstance(r['unresolved_name'], str)
        else:
            assert r['target_ref'] in entity_ids and r['unresolved_name'] is None
        if r['range'] is not None:
            span(r['range'])
        source = entities[r['source_ref']]
        target = entities.get(r['target_ref']) if r['target_ref'] is not None else None
        if r['kind'] == 'contains':
            assert r['resolution'] == 'direct_structural'
            assert r['metadata']['kind'] == 'none'
            assert target is not None and target[0] in {'file', 'symbol'}
            if r['derivation'] == 'repository_contains_file':
                assert source[0] == 'repository' and target[0] == 'file'
                assert r['path'] == target[1] and r['range'] is None
            else:
                assert r['derivation'] == 'lexical_range_containment'
                assert target[0] == 'symbol' and target[1] == r['path']
                if source[0] == 'file':
                    assert source[1] == r['path'] and r['range'] == target[2]['range']
                else:
                    assert source[0] == 'symbol' and source[1] == r['path']
                    assert source[2]['range']['start']['byte'] <= target[2]['range']['start']['byte']
                    assert target[2]['range']['end']['byte'] <= source[2]['range']['end']['byte']
        elif r['kind'] == 'imports':
            assert r['derivation'] in {'rust_use_syntax', 'python_import_syntax'}
            assert source[0] == 'file' and source[1] == r['path']
            assert r['resolution'] == 'unresolved' and target is None
            assert r['metadata']['kind'] == 'import'
        else:
            assert r['kind'] == 'references'
            assert r['derivation'] in {'rust_call_syntax', 'python_call_syntax'}
            assert source[0] == 'symbol' and source[1] == r['path']
            assert r['metadata'] == {'kind': 'reference', 'context': 'call'}
            if target is not None:
                assert target[0] == 'symbol' and target[1] == r['path']
            else:
                assert r['resolution'] == 'unresolved'
    counts = coverage['counts']
    for k in ['files', 'symbols', 'relations']:
        assert counts[k + '_exported'] == len(d[k])
    assert counts['files_observed'] == len(d['files']) + counts.get('unrepresentable_files_omitted', 0)
    assert counts['files_observed'] == sum(counts.get('analysis_' + s, 0) for s in STATUS)
    for status in STATUS:
        assert counts.get('analysis_' + status, 0) >= sum(f['analysis_status'] == status for f in d['files'])
    assert counts.get('syntax_errors', 0) >= sum(f['syntax_errors'] for f in d['files'])
    return entity_ids


def validate(raw):
    assert len(raw) <= MAX
    d = json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    check_schema(d, SCHEMA)
    assert type(d['evidence_schema_version']) is int and d['evidence_schema_version'] == 1
    o = d['observation']; c = d['coverage']; data = d['data']; feed = d['feed']
    assert o['atomic'] is False and c['complete'] is False
    assert re.fullmatch('[a-f0-9]{64}', o['scope'])
    assert all(type(v) is int and v >= 0 for v in c['counts'].values())
    for key in ['git_head', 'base', 'head']:
        assert o[key] is None or re.fullmatch('[a-f0-9]{40}|[a-f0-9]{64}', o[key])
    assert o['git_dirty'] is None or type(o['git_dirty']) is bool
    assert d['subject']['kind'] == ('workspace' if feed == 'workspace' else 'repository')
    logical = d['subject']['workspace_id' if feed == 'workspace' else 'repository_id']
    assert re.fullmatch('[a-z][a-z0-9_-]{0,63}', logical)
    if feed in ['structure', 'candidates']:
        assert o['basis'] == 'best_effort_worktree' and o['base'] is None and o['head'] is None
    if feed == 'structure':
        structure(data, c)
    elif feed == 'changes':
        assert o['basis'] == 'committed_changed_pairs' and o['base'] and o['head']
        base = structure(data['base'], data['base_coverage']); head = structure(data['head'], data['head_coverage'])
        if o['base'] != o['head']:
            assert not base.intersection(head)
        assert c['counts']['changed_files'] == len(data['files'])
        for f in data['files']:
            for side in ['before', 'after']:
                if f[side] is not None:
                    safe(f[side]['path'])
                    assert re.fullmatch('[a-f0-9]{40}|[a-f0-9]{64}', f[side]['object_id'])
            for s in f['symbols']:
                assert s['before_ref'] is None or s['before_ref'] in base
                assert s['after_ref'] is None or s['after_ref'] in head
                if s['before_ref'] is not None:
                    assert s['before_ref'] in base
                if s['after_ref'] is not None:
                    assert s['after_ref'] in head
            for r in f['relations']:
                assert r['resolution'] in RESOLUTION and r['derivation'] in DERIVATION
                for side in ['before', 'after']:
                    occ = r[side]
                    assert occ['count'] == len(occ['examples']) + occ['examples_omitted']
                    for example in occ['examples']:
                        span(example)
    elif feed == 'candidates':
        assert data['ranker_version'] == 'file-ranker-v1'
        assert data['semantics'] == 'ranked_metadata_not_content_selection'
        ids = set(); paths = set(); ranks = []
        for x in data['candidates']:
            file(x['file']); f = x['file']
            assert f['path'] not in paths and f['id'] not in ids
            paths.add(f['path']); ids.add(f['id']); ranks.append(x['rank'])
            assert x['score']['total'] == sum(v for k, v in x['score'].items() if k != 'total')
            assert x['reasons'] == sorted(set(x['reasons']))
            for s in x['matched_symbols']:
                symbol(s, {f['id']: f}); assert s['id'] not in ids; ids.add(s['id'])
        assert ranks == sorted(set(ranks)) and all(type(r) is int and 1 <= r <= c['counts']['ranking_candidates'] for r in ranks)
        n = c['counts']; assert n['candidates_exported'] == len(ranks)
        assert n['ranking_candidates'] == len(ranks) + n.get('unrepresentable_candidates_omitted', 0) + n['candidate_limit_omitted']
        if data['change']:
            assert o['git_dirty'] is False and o['git_head'] == data['change']['head']
    elif feed == 'workspace':
        assert o['basis'] == 'mixed_time_workspace'
        assert data['workspace_schema_version'] == 2 and data['manifest_schema_version'] == 1
        repos = {r['id'] for r in data['repositories']}; assert len(repos) == len(data['repositories'])
        ids = set()
        for r in data['relations']:
            assert r['provider'] in repos and r['consumer'] in repos and r['id'] not in ids
            ids.add(r['id'])
            assert r['kind'] in {'cargo_path_dependency', 'declared_contract_consumer'}
            assert r['basis'] == ('observed_manifest_dependency' if r['kind'] == 'cargo_path_dependency' else 'declared_workspace_mapping')
            for key in ['provider_path', 'consumer_path', 'source_file']:
                if r[key] is not None: safe(r[key])
        if data['impact']:
            relations = data['relations']
            for x in data['impact']['candidates']:
                assert x['repo'] in repos and len(x['direct_triggers']) <= 64
                assert all(t['relation_id'] in ids for t in x['direct_triggers'])
                by_id = {r['id']: r for r in relations}
                assert all(by_id[t['relation_id']]['consumer'] == x['repo'] for t in x['direct_triggers'])
                assert all(type(e) is int and 0 <= e < len(relations) for e in x['witness_edges'])
                chain = [relations[e] for e in x['witness_edges']]
                assert all(a['consumer'] == b['provider'] for a, b in zip(chain, chain[1:]))
                if chain:
                    assert chain[-1]['consumer'] == x['repo']
    else:
        raise ValueError('feed')
    return d


if __name__ == '__main__':
    try:
        validate(sys.stdin.buffer.read(MAX + 1))
    except (AssertionError, ValueError, KeyError, TypeError, UnicodeError, RecursionError):
        sys.exit(1)
