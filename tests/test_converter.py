import tempfile
import unittest
from pathlib import Path

from core.converter import (
	VIDEO_SUFFIX,
	build_image_args,
	build_video_args,
	collect_media_files,
	image_output_path,
	video_output_path,
)


class ConverterTests(unittest.TestCase):
	def test_collects_folder_videos_without_generated_outputs_or_nested_files(self):
		with tempfile.TemporaryDirectory() as tmp:
			root = Path(tmp)
			(root / "b.mov").write_text("", encoding="utf-8")
			(root / "a.mp4").write_text("", encoding="utf-8")
			(root / f"done{VIDEO_SUFFIX}.mp4").write_text("", encoding="utf-8")
			(root / "nested").mkdir()
			(root / "nested" / "c.mkv").write_text("", encoding="utf-8")

			files = collect_media_files(root, "video")

		self.assertEqual([path.name for path in files], ["a.mp4", "b.mov"])

	def test_collects_only_images_that_need_jpg_outputs(self):
		with tempfile.TemporaryDirectory() as tmp:
			root = Path(tmp)
			(root / "a.png").write_text("", encoding="utf-8")
			(root / "b.jpeg").write_text("", encoding="utf-8")
			(root / "c.jpg").write_text("", encoding="utf-8")

			files = collect_media_files(root, "image")

		self.assertEqual([path.name for path in files], ["a.png", "b.jpeg"])

	def test_output_paths_are_unique_and_keep_originals(self):
		with tempfile.TemporaryDirectory() as tmp:
			root = Path(tmp)
			video = root / "clip.mp4"
			video.write_text("", encoding="utf-8")
			(root / f"clip{VIDEO_SUFFIX}.mp4").write_text("", encoding="utf-8")
			image = root / "photo.jpeg"
			image.write_text("", encoding="utf-8")
			(root / "photo.jpg").write_text("", encoding="utf-8")

			self.assertEqual(
				video_output_path(video).name, f"clip{VIDEO_SUFFIX}-2.mp4"
			)
			self.assertEqual(image_output_path(image).name, "photo-2.jpg")

	def test_video_args_enforce_requested_compatibility_format(self):
		args = build_video_args("in.mov", "out.mp4", "ffmpeg.exe")

		self.assertIn("libx264", args)
		self.assertIn("aac", args)
		self.assertIn("scale=-2:min(1080\\,ih)", args)
		self.assertEqual(args[-1], "out.mp4")

	def test_image_args_write_one_high_quality_jpg_frame(self):
		args = build_image_args("in.png", "out.jpg", "ffmpeg.exe")

		self.assertIn("format=rgb24", args)
		self.assertIn("2", args)
		self.assertEqual(args[-1], "out.jpg")


if __name__ == "__main__":
	unittest.main()
