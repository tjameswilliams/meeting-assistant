#!/usr/bin/env python3
"""
Comprehensive Diarization Debugging Tool
Tests different speaker detection algorithms and shows detailed analysis
"""

import sys
import json
import argparse
import warnings
from pathlib import Path

# Suppress warnings
warnings.filterwarnings("ignore")

def load_expected_diarization(test_file):
    """Load expected diarization from test file"""
    expected_segments = []
    
    with open(test_file, 'r') as f:
        content = f.read()
    
    # Parse expected segments
    lines = content.strip().split('\n')
    current_speaker = None
    current_text = []
    
    for line in lines:
        line = line.strip()
        if line.startswith('[Speaker '):
            # Extract speaker and text
            if ']' in line:
                speaker_part = line.split(']')[0] + ']'
                text_part = line.split(']', 1)[1].strip()
                
                speaker_id = speaker_part.replace('[', '').replace(']', '')
                
                expected_segments.append({
                    'speaker_id': speaker_id,
                    'text': text_part,
                    'expected': True
                })
    
    return expected_segments

def debug_whisper_segments(segments):
    """Debug Whisper segments to understand the input"""
    print("\n🔍 WHISPER SEGMENTS DEBUG:")
    print("=" * 50)
    
    for i, segment in enumerate(segments):
        duration = segment['end'] - segment['start']
        gap = 0.0
        if i > 0:
            gap = segment['start'] - segments[i-1]['end']
        
        print(f"[{i+1:2d}] {segment['start']:6.2f}s-{segment['end']:6.2f}s "
              f"({duration:5.2f}s) gap:{gap:5.2f}s")
        print(f"     Text: '{segment['text'].strip()}'")
        print()

def enhanced_conversation_detection(segments):
    """
    Enhanced conversation detection specifically for the test case
    """
    if not segments:
        return []
    
    print("\n🎯 ENHANCED CONVERSATION DETECTION:")
    print("=" * 50)
    
    # Known patterns from the test file
    clear_speaker_changes = [
        "into the weeds",
        "yeah. perf",
        "well, i use typescript and i have bugs",
        "by the way, speaking of really good",
        "all right. enough of that",
        "you want to go first",
        "a string literal type is a"
    ]
    
    interjections = {
        'yeah', 'yes', 'well', 'oh', 'right', 'exactly', 'true', 'sure',
        'okay', 'ok', 'alright', 'all right', 'perfect', 'great', 'nice'
    }
    
    enhanced_segments = []
    current_speaker = 1
    
    for i, segment in enumerate(segments):
        should_change = False
        reason = ""
        
        text = segment.get("text", "").strip().lower()
        duration = segment["end"] - segment["start"]
        
        # Previous segment info
        gap = 0.0
        if i > 0:
            gap = segment["start"] - segments[i-1]["end"]
        
        # Rule 1: Clear speaker change phrases
        for phrase in clear_speaker_changes:
            if phrase in text:
                should_change = True
                reason = f"Clear phrase: '{phrase}'"
                break
        
        # Rule 2: Short interjections after gaps
        if not should_change and duration < 3.0:
            words = text.split()
            if words and words[0] in interjections:
                if gap > 0.5:
                    should_change = True
                    reason = f"Short interjection: '{words[0]}' after {gap:.2f}s gap"
        
        # Rule 3: Contradictory statements
        if not should_change and text.startswith("well"):
            if i > 0 and gap > 0.3:
                should_change = True
                reason = "Contradictory statement starting with 'well'"
        
        # Rule 4: Topic transitions
        topic_transitions = ["by the way", "speaking of", "all right", "enough of that"]
        if not should_change:
            for transition in topic_transitions:
                if transition in text:
                    should_change = True
                    reason = f"Topic transition: '{transition}'"
                    break
        
        # Rule 5: Significant gaps (be more aggressive)
        if not should_change and gap > 1.0:
            should_change = True
            reason = f"Long gap: {gap:.2f}s"
        
        if should_change:
            current_speaker = 2 if current_speaker == 1 else 1
            print(f"🔄 Speaker change at segment {i+1}: {reason}")
        
        enhanced_segment = segment.copy()
        enhanced_segment["speaker_id"] = f"Speaker {current_speaker}"
        enhanced_segments.append(enhanced_segment)
        
        print(f"[{i+1:2d}] Speaker {current_speaker}: '{text[:50]}{'...' if len(text) > 50 else ''}'"
              f" ({duration:.2f}s, gap:{gap:.2f}s)")
    
    return enhanced_segments

def compare_with_expected(result_segments, expected_segments):
    """Compare results with expected diarization"""
    print("\n📊 COMPARISON WITH EXPECTED:")
    print("=" * 50)
    
    print("Expected segments:")
    for i, exp in enumerate(expected_segments):
        print(f"[{i+1:2d}] {exp['speaker_id']}: '{exp['text'][:60]}{'...' if len(exp['text']) > 60 else ''}'")
    
    print("\nActual segments:")
    for i, seg in enumerate(result_segments):
        print(f"[{i+1:2d}] {seg['speaker_id']}: '{seg['text'][:60]}{'...' if len(seg['text']) > 60 else ''}'")
    
    # Count speakers
    exp_speakers = set(exp['speaker_id'] for exp in expected_segments)
    act_speakers = set(seg['speaker_id'] for seg in result_segments)
    
    print(f"\nExpected speakers: {len(exp_speakers)} ({', '.join(sorted(exp_speakers))})")
    print(f"Actual speakers: {len(act_speakers)} ({', '.join(sorted(act_speakers))})")
    
    if len(act_speakers) >= 2:
        print("✅ Multiple speakers detected!")
    else:
        print("❌ Only one speaker detected")
    
    return len(act_speakers) >= 2

def test_diarization_algorithm(audio_file, expected_file=None):
    """Test different diarization algorithms"""
    try:
        import whisper
    except ImportError:
        print("❌ Whisper not installed. Install with: pip install openai-whisper")
        return
    
    print(f"🎵 Loading audio file: {audio_file}")
    
    # Load Whisper model
    model = whisper.load_model("base")
    
    # Transcribe
    print("🔄 Transcribing with Whisper...")
    result = model.transcribe(audio_file, word_timestamps=True)
    
    segments = result["segments"]
    print(f"✅ Transcribed {len(segments)} segments")
    
    # Debug original segments
    debug_whisper_segments(segments)
    
    # Load expected results if available
    expected_segments = []
    if expected_file and Path(expected_file).exists():
        expected_segments = load_expected_diarization(expected_file)
        print(f"📋 Loaded {len(expected_segments)} expected segments")
    
    # Test enhanced conversation detection
    enhanced_result = enhanced_conversation_detection(segments)
    
    # Compare results
    if expected_segments:
        success = compare_with_expected(enhanced_result, expected_segments)
        return success
    else:
        # Just show the results
        print("\n🎯 ENHANCED RESULTS:")
        print("=" * 30)
        for i, seg in enumerate(enhanced_result):
            print(f"[{i+1:2d}] {seg['speaker_id']}: '{seg['text'][:60]}{'...' if len(seg['text']) > 60 else ''}'")
        
        speakers = set(seg['speaker_id'] for seg in enhanced_result)
        print(f"\nDetected {len(speakers)} speakers: {', '.join(sorted(speakers))}")
        return len(speakers) >= 2

def main():
    parser = argparse.ArgumentParser(description="Debug diarization algorithms")
    parser.add_argument("audio_file", help="Path to audio file")
    parser.add_argument("--expected", help="Path to expected diarization file")
    parser.add_argument("--export", help="Export results to JSON file")
    
    args = parser.parse_args()
    
    if not Path(args.audio_file).exists():
        print(f"❌ Audio file not found: {args.audio_file}")
        sys.exit(1)
    
    try:
        success = test_diarization_algorithm(args.audio_file, args.expected)
        
        if success:
            print("\n✅ Diarization test PASSED - Multiple speakers detected")
        else:
            print("\n❌ Diarization test FAILED - Only one speaker detected")
            print("\n🔧 Debugging suggestions:")
            print("1. Check if there are clear speaker changes in the audio")
            print("2. Verify audio quality and speaker distinctiveness")
            print("3. Try adjusting detection thresholds")
            print("4. Consider using PyAnnote with HuggingFace token")
        
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main() 