"""Pinned producer-serialized descriptor fixtures; no runtime/inference proof."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator
import local_http_session as consumer

DIRECTORY = Path(__file__).parent
FIXTURES = json.loads((DIRECTORY / 'fixtures/image-to-text-descriptors.json').read_text())
SCHEMA_BYTES = (DIRECTORY / 'contracts/capability-descriptor.schema.json').read_bytes()
SCHEMA = json.loads(SCHEMA_BYTES)
VALIDATOR = Draft202012Validator(SCHEMA)


def fixture_build():
    # Deliberately controlled advertisement, not emitted by local native binary.
    return {'component': 'pumas-rpc', 'build_info_schema_version': 1,
            'protocols': [{'name': 'pumas.local-http', 'versions': [1]}],
            'schemas': [{'name': name, 'version': 1} for name in (
                'pumas.http-advertisement', 'pumas.http-admission-fence',
                'pumas.http-owner-retention', 'pumas.model-operations.image-to-text')]}


class ProducerDescriptorFixtures(unittest.TestCase):
    def setUp(self):
        self.available = copy.deepcopy(FIXTURES['available_descriptor_fixture'])
        self.unavailable = copy.deepcopy(FIXTURES['unavailable_descriptor_fixture'])
        self.build = fixture_build()

    def decode(self, value, **kwargs):
        return consumer.decode_capability_descriptor(json.dumps(value).encode(),
            build_info=kwargs.get('build_info', self.build), contract_version=kwargs.get('contract_version', 1))

    def select(self, values, **kwargs):
        return consumer.select_capability(values, build_info=self.build, contract_version=1,
            input_modality=kwargs.get('input_modality', 'image'),
            output_modality=kwargs.get('output_modality', 'text'),
            semantic_task=kwargs.get('semantic_task'), stream=kwargs.get('stream', False))

    def test_exact_original_schema_and_both_producer_serialized_descriptors(self):
        self.assertEqual(len(SCHEMA_BYTES), 3537)
        self.assertEqual(hashlib.sha256(SCHEMA_BYTES).hexdigest(), consumer.DESCRIPTOR_SCHEMA_SHA256)
        Draft202012Validator.check_schema(SCHEMA)
        for value in (self.available, self.unavailable):
            VALIDATOR.validate(value)
            self.assertEqual(self.decode(value), value)
        self.assertEqual(self.select([self.available]), self.available)
        with self.assertRaisesRegex(ValueError, 'unavailable'):
            self.select([self.unavailable])

    def test_schema_permitted_extra_properties_are_retained_and_detached(self):
        value = self.available
        value['extension'] = {'nested': ['preserve this observation']}
        value['availability']['extension'] = ['future']
        value['option_bounds'][0]['extension'] = {'detail': 3}
        VALIDATOR.validate(value)
        decoded = self.decode(value)
        self.assertEqual(decoded, value)
        selected = self.select([decoded])
        selected['extension']['nested'].append('local edit')
        self.assertEqual(decoded, value)
        self.assertEqual(decoded['extension']['nested'], ['preserve this observation'])

    def test_closed_discriminants_and_required_fields_match_actual_schema(self):
        cases = []
        for field in SCHEMA['required']:
            value = copy.deepcopy(self.available);value.pop(field);cases.append(value)
        for field in ('capability', 'semantic_task'):
            for bad in ('future-value', True, None):
                value = copy.deepcopy(self.available);value[field] = bad;cases.append(value)
        for field in ('input_formats', 'output_formats'):
            for bad in ('text', ['future-format'], [1], [None]):
                value = copy.deepcopy(self.available);value[field] = bad;cases.append(value)
        for bad in (1, 'true', None):
            value = copy.deepcopy(self.available);value['streaming'] = bad;cases.append(value)
        for bad in ({'state': 'future'}, {'state': True}, {'state': 'unavailable'},
                    {'state': 'unavailable', 'reason': 'future'}, {'state': 'unavailable', 'reason': False}):
            value = copy.deepcopy(self.available);value['availability'] = bad;cases.append(value)
        for field, bad in (('option', 'future'), ('minimum', True), ('maximum', '2')):
            value = copy.deepcopy(self.available);value['option_bounds'][0][field] = bad;cases.append(value)
        value = copy.deepcopy(self.available);value['option_bounds'][0].pop('minimum');cases.append(value)
        for value in cases:
            with self.subTest(value=value):
                self.assertFalse(VALIDATOR.is_valid(value))
                with self.assertRaises(ValueError):
                    self.decode(value)

    def test_schema_hash_contract_version_and_compiled_marker_refusals(self):
        for version in (True, '1', 2):
            with self.assertRaises(ValueError):self.decode(self.available, contract_version=version)
        for replacement in (None, 2, True):
            build = copy.deepcopy(self.build)
            item = build['schemas'][-1]
            if replacement is None:build['schemas'].remove(item)
            else:item['version'] = replacement
            with self.assertRaises(ValueError):self.decode(self.available, build_info=build)
        build = copy.deepcopy(self.build);build['schemas'].append(build['schemas'][-1])
        with self.assertRaises(ValueError):self.decode(self.available, build_info=build)
        with patch.object(Path, 'read_bytes', return_value=SCHEMA_BYTES+b' '):
            with self.assertRaisesRegex(ValueError, 'schema changed'):self.decode(self.available)

    def test_duplicate_keys_overflow_and_host_size_bound(self):
        base = json.dumps(self.available).encode()
        for data in (b'{"capability":"image_to_text",'+base[1:],
                     base.replace(b'2048.0', b'1e999'),
                     base.replace(b'2048.0', b'NaN'),
                     b' '*(consumer.MAX_RPC+1)):
            with self.assertRaises(ValueError):
                consumer.decode_capability_descriptor(data, build_info=self.build, contract_version=1)
        value = copy.deepcopy(self.available);value['extension'] = float('inf')
        with self.assertRaises(ValueError):self.select([value])

    def test_generic_format_selection_ignores_capability_names(self):
        # Controlled observations derived from pinned field enums, not DTO-emitted
        # runtime availability. Selection depends on declared formats/state.
        for capability in SCHEMA['$defs']['Capability']['enum']:
            value = copy.deepcopy(self.available);value['capability'] = capability
            self.assertEqual(self.select([value])['capability'], capability)
        for input_modality, inputs, output_modality, outputs in (
                ('text', ['text', 'messages_text', 'text_batch'], 'text', ['text']),
                ('text', ['text'], 'embeddings', ['embeddings_float32']),
                ('text', ['text'], 'image', ['png_base64']),
                ('audio', ['pcm_s16le', 'pcm_f32le'], 'labels', ['labels'])):
            value = copy.deepcopy(self.available);value.update(input_formats=inputs, output_formats=outputs)
            self.assertEqual(self.select([value], input_modality=input_modality,
                                        output_modality=output_modality), value)

    def test_ambiguity_requires_semantic_task_and_options_never_select_it(self):
        chat = copy.deepcopy(self.available)
        chat.update(capability='chat_generation', semantic_task='chat_generation', input_formats=['messages_text'])
        completion = copy.deepcopy(chat)
        completion.update(capability='text_generation', semantic_task='text_generation', input_formats=['text'])
        completion['option_bounds'] = []
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            self.select([chat, completion], input_modality='text')
        self.assertEqual(self.select([chat, completion], input_modality='text', semantic_task='text_generation'), completion)
        completion['availability'] = {'state': 'unavailable', 'reason': 'runtime_unavailable'}
        self.assertEqual(self.select([chat, completion], input_modality='text'), chat)

    def test_unsupported_unavailable_streaming_and_selector_consistency_policies(self):
        with self.assertRaisesRegex(ValueError, 'no declared'):
            self.select([self.available], input_modality='audio')
        with self.assertRaisesRegex(ValueError, 'no declared'):
            self.select([self.available], stream=True)
        with self.assertRaises(ValueError):self.select([self.available], stream=1)
        with self.assertRaises(ValueError):self.select([self.available], semantic_task='future')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            self.select([self.available, self.unavailable])
        for changed in ('duplicate', 'reversed'):
            value = copy.deepcopy(self.available)
            if changed=='duplicate':value['option_bounds'].append(value['option_bounds'][0])
            else:value['option_bounds'][0].update(minimum=4, maximum=2)
            # Valid producer schema shape, refused by the host selector's
            # explicit policy for contradictory bounds; not a schema claim.
            VALIDATOR.validate(value);self.decode(value)
            with self.assertRaisesRegex(ValueError, 'bounds'):self.select([value])

    def test_empty_and_repeated_format_arrays_follow_schema(self):
        value = copy.deepcopy(self.available)
        value['input_formats'] = []
        VALIDATOR.validate(value);self.decode(value)
        with self.assertRaisesRegex(ValueError, 'no declared'):self.select([value])
        value['input_formats'] = ['png_base64', 'png_base64']
        VALIDATOR.validate(value)
        self.assertEqual(self.select([self.decode(value)]), value)
