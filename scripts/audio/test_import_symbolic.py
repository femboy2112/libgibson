"""Importer calibration with synthetic controls; no melody transcribed from memory."""
import tempfile
import unittest
from pathlib import Path
import import_symbolic as importer


class LilyTests(unittest.TestCase):
    def test_absolute_pitch_transpose_duration_rest_and_tie(self):
        source = "line = \\transpose c d { c'4. d'8 r4 e'4~ e'4 f'2 }"
        voice = importer.lily_voice(source, 'line', 'synthetic')
        self.assertEqual(voice['length'], [7, 1])
        self.assertEqual([n['pitch'] for n in voice['notes']], [62, 64, 66, 67])
        self.assertEqual([n['onset'] for n in voice['notes']], [[0,1],[3,2],[3,1],[5,1]])
        self.assertEqual([n['duration'] for n in voice['notes']], [[3,2],[1,2],[2,1],[2,1]])
        self.assertEqual(len(voice['notes'][2]['origins']), 2)

    def test_unsupported_syntax_is_not_silently_skipped(self):
        for body in [r'\relative c { c4 d }', r'\tuplet 3/2 { c8 d e }',
                     'c4 <e g>4', r'c4 \unknown d4', 'c4~ d4', 'c4~ r4',
                     'c4~', 'c3', r'\repeat volta 2 { c4 }']:
            with self.subTest(body=body), self.assertRaises(ValueError):
                importer.lily_voice('line = { '+body+' }', 'line', 'synthetic')

    def test_missing_metadata_stays_unknown(self):
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'input.ly'
            p.write_text("line = { c'4 d' e' f' }")
            obs=importer.lily_observations(p,['line'])
            self.assertIsNone(obs['key'])
            self.assertIsNone(obs['meter'])
            self.assertIsNone(obs['quarter_bpm'])
            self.assertIsNone(obs['harmony'])

    def test_metadata_changes_are_not_flattened(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'input.ly'
            for metadata in [r'\time 4/4 \time 3/4', r'\key c \major \key d \major',
                             r'\tempo 4=100 \tempo 4=80']:
                p.write_text("line = { c'4 d' e' f' }\nglobal = { " + metadata + ' }')
                with self.assertRaises(ValueError):
                    importer.lily_observations(p, ['line'])

    def test_pitch_and_rhythm_mutation_break_cross_check(self):
        a=importer.lily_voice('line = { c4 d4 }','line','a')['notes']
        for source in ['line = { c4 e4 }','line = { c8 d4 }']:
            b=importer.lily_voice(source,'line','b')['notes']
            self.assertNotEqual(importer.signature(a),importer.signature(b))


class MidiTests(unittest.TestCase):
    def test_note_on_zero_and_delta_accumulation(self):
        import mido
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'test.mid'
            f=mido.MidiFile(type=1,ticks_per_beat=96)
            track=mido.MidiTrack();f.tracks.append(track)
            track.extend([mido.Message('note_on',note=60,velocity=80,time=24),
                          mido.Message('note_on',note=60,velocity=0,time=48)])
            f.save(p)
            obs=importer.midi_observations(p)
            n=obs['tracks'][0]['notes'][0]
            self.assertEqual(n['onset'],[1,4])
            self.assertEqual(n['duration'],[1,2])
            self.assertEqual(n['velocity'],80)
            self.assertEqual(obs['metadata'],[])

    def test_unterminated_or_asynchronous_source_rejected(self):
        import mido
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'test.mid'
            for kind in [1,2]:
                f=mido.MidiFile(type=kind)
                track=mido.MidiTrack();f.tracks.append(track)
                track.append(mido.Message('note_on',note=60,velocity=80))
                f.save(p)
                with self.assertRaises(ValueError):importer.midi_observations(p)


if __name__=='__main__':
    unittest.main()
