import os
import sys
import time
import argparse
from datetime import timedelta
from faster_whisper import WhisperModel
import yt_dlp


def download_audio(url: str, output_path: str = "audio") -> str:
    print(f"🔽 Downloading audio from: {url}")
    
    ydl_opts = {
        'format': 'bestaudio/best',
        'outtmpl': f'{output_path}/%(id)s.%(ext)s',
        'quiet': False,
        'no_warnings': False,
        'extract_audio': True,
        'postprocessors': [{
            'key': 'FFmpegExtractAudio',
            'preferredcodec': 'mp3',
            'preferredquality': '192',
        }],
        'http_headers': {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        },
    }

    os.makedirs(output_path, exist_ok=True)

    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(url, download=True)
        audio_file = ydl.prepare_filename(info)
        if not audio_file.endswith('.mp3'):
            audio_file = os.path.splitext(audio_file)[0] + '.mp3'
        print(f"✅ Audio downloaded: {audio_file}")
        return audio_file


def format_timestamp(seconds: float) -> str:
    return str(timedelta(seconds=int(seconds)))[2:]


def transcribe_audio(audio_path: str, model_size: str = "large-v3-turbo", timestamped: bool = True):
    print(f"🧠 Loading Whisper model: {model_size} ...")

    device = "cuda" if os.path.exists("/usr/local/cuda") or os.name == "nt" else "cpu"
    compute_type = "float16" if device == "cuda" else "int8"

    start_time = time.time()

    try:
        model = WhisperModel(model_size, device=device, compute_type=compute_type)

        print("🎙️  Starting transcription...")
        segments, info = model.transcribe(
            audio_path,
            beam_size=5,
            word_timestamps=True,
            vad_filter=True,
            language=None
        )

        print(f"🌍 Detected language: {info.language}")

        base_name = os.path.splitext(audio_path)[0]
        
        # Always save plain transcript
        txt_path = base_name + "_transcript.txt"
        
        # Timestamped version (if flag is enabled)
        timestamped_path = base_name + "_timestamped.txt"
        
        # SRT always
        srt_path = base_name + "_subtitles.srt"

        with open(txt_path, "w", encoding="utf-8") as f, open(srt_path, "w", encoding="utf-8") as sf:
            if timestamped:
                with open(timestamped_path, "w", encoding="utf-8") as tf:
                    for i, segment in enumerate(segments, 1):
                        ts = format_timestamp(segment.start)
                        text = segment.text.strip()

                        f.write(f"{text}\n")
                        tf.write(f"[{ts}] {text}\n")

                        start_str = str(timedelta(seconds=int(segment.start)))
                        end_str = str(timedelta(seconds=int(segment.end)))
                        sf.write(f"{i}\n{start_str} --> {end_str}\n{text}\n\n")
            else:
                for i, segment in enumerate(segments, 1):
                    text = segment.text.strip()
                    f.write(f"{text}\n")

                    start_str = str(timedelta(seconds=int(segment.start)))
                    end_str = str(timedelta(seconds=int(segment.end)))
                    sf.write(f"{i}\n{start_str} --> {end_str}\n{text}\n\n")

        print(f"\n✅ Transcription Completed!")
        print(f"📄 Plain Transcript     : {txt_path}")
        if timestamped:
            print(f"⏱️  Timestamped Transcript: {timestamped_path}")
        print(f"🎞️  Subtitles (SRT)      : {srt_path}")

        # Preview
        preview_file = timestamped_path if timestamped else txt_path
        print(f"\n--- Preview ---")
        with open(preview_file, "r", encoding="utf-8") as f:
            print("".join(f.readlines()[:10]))

    except Exception as e:
        print(f"❌ Error: {e}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Transcribe video/audio from URL")
    parser.add_argument("url", help="Video URL (YouTube, Instagram, etc.)")
    parser.add_argument("--no-timestamp", action="store_true", 
                       help="Disable timestamps (plain transcript only)")
    parser.add_argument("--model", default="large-v3-turbo", 
                       choices=["small", "medium", "large-v3-turbo", "large-v3"],
                       help="Whisper model size")
    
    args = parser.parse_args()

    try:
        audio_file = download_audio(args.url)
        transcribe_audio(
            audio_file, 
            model_size=args.model, 
            timestamped=not args.no_timestamp
        )
    except Exception as e:
        print(f"💥 Error: {e}")
