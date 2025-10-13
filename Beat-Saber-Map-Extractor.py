import os
import gzip
import UnityPy
import shutil
import sys
import threading
import queue
import customtkinter as ctk
from tkinter import filedialog, messagebox


def resource_path(relative_path):
    """ Get absolute path to resource (works for dev & for PyInstaller) """
    base = getattr(sys, "_MEIPASS", None)
    if base:
        # PyInstaller extracts to _MEIPASS dir
        return os.path.join(base, relative_path)
    return os.path.join(os.path.abspath("."), relative_path)

# ----------------- GZ.Asset to .DAT Conversion -----------------
def _is_windows():
    return os.name == "nt"

# ----------------- GUI Setup -----------------
ctk.set_default_color_theme("blue")

class ModernUI(ctk.CTk):
    def __init__(self):
        super().__init__()
        
        icon_path = resource_path("beat-saber-map-extractor-icon.ico")
        try:
            self.iconbitmap(default=icon_path)
        except Exception:
            # Don't crash if icon is missing (e.g., during dev runs)
            pass


        # Window properties
        self.title("Beat Saber Mapping Extractor")
        
        self.geometry("1000x600")  # Starting size
        self.minsize(600, 400)    # Minimum size allowed
        self.resizable(True, True) # Allow resizing (default is True but explicit is good)

        # Shared state and thread comms
        self._threads = []
        self._ui_queue = queue.Queue()
        self._cancel_event = threading.Event()


        # Sidebar
        self.sidebar = ctk.CTkFrame(self, width=200, corner_radius=0)
        self.sidebar.pack(side="left", fill="y")

        # Sidebar buttons
        self.logo_label = ctk.CTkLabel(self.sidebar, text="Main Menu", font=("Segoe UI Bold", 20))
        self.logo_label.pack(pady=20)

        self.converter_button = ctk.CTkButton(self.sidebar, text="Converter", corner_radius=50, command=self.show_converter)
        self.converter_button.pack(pady=10, padx=10, fill="x")

        self.extractor_button = ctk.CTkButton(self.sidebar, text="Extractor", corner_radius=50, command=self.show_extractor)
        self.extractor_button.pack(pady=10, padx=10, fill="x")

        # self.settings_button = ctk.CTkButton(self.sidebar, text="Settings", corner_radius=50, command=self.show_settings)
        # self.settings_button.pack(pady=10, padx=10, fill="x")

        # Bottom version label
        self.version_label = ctk.CTkLabel(self.sidebar, text="v0.6.0", font=("Segoe UI", 10))
        self.version_label.pack(side="bottom", pady=20)

        # Content Frame
        self.content = ctk.CTkFrame(self, corner_radius=0)
        self.content.pack(side="right", expand=True, fill="both")

        # Show Converter tab initially
        self.show_converter()

    def clear_content(self):
        for widget in self.content.winfo_children():
            widget.destroy()

    def show_converter(self):
        self.clear_content()
        ctk.CTkLabel(self.content, text="GZ.Asset to .DAT Converter", font=("Segoe UI", 24)).pack(pady=20)
        
        # Vars
        self.input_folder_var = ctk.StringVar()
        self.output_folder_var = ctk.StringVar()

        ctk.CTkLabel(self.content, text="Input Folder:").pack(pady=5)
        entry_in = ctk.CTkEntry(self.content, textvariable=self.input_folder_var, corner_radius=50, width=500)
        entry_in.pack(pady=5)
        ctk.CTkButton(self.content, text="Browse", corner_radius=50, command=self.browse_input_folder).pack(pady=5)

        ctk.CTkLabel(self.content, text="Output Folder:").pack(pady=5)
        entry_out = ctk.CTkEntry(self.content, textvariable=self.output_folder_var, corner_radius=50, width=500)
        entry_out.pack(pady=5)
        ctk.CTkButton(self.content, text="Browse", corner_radius=50, command=self.browse_output_folder).pack(pady=5)
        
        # Controls row
        btn_frame = ctk.CTkFrame(self.content)
        btn_frame.pack(pady=10)
        self.start_conv_btn = ctk.CTkButton(btn_frame, text="Start Conversion", corner_radius=50, command=self.start_conversion)
        self.start_conv_btn.pack(side="left", padx=5)
        self.cancel_conv_btn = ctk.CTkButton(btn_frame, text="Cancel", corner_radius=50, command=self.cancel_current, state="disabled")
        self.cancel_conv_btn.pack(side="left", padx=5)
        self.open_conv_btn = ctk.CTkButton(btn_frame, text="Open Output Folder", corner_radius=50, command=lambda: self.open_folder(self.output_folder_var.get()), state="disabled")
        self.open_conv_btn.pack(side="left", padx=5)

        # Progress + status
        self.conv_progress = ctk.CTkProgressBar(self.content)
        self.conv_progress.set(0)
        self.conv_progress.pack(pady=10, fill="x", padx=20)
        self.conv_status = ctk.CTkLabel(self.content, text="Idle")
        self.conv_status.pack(pady=5)

        # Log
        self.log_box = ctk.CTkTextbox(self.content, height=200)
        self.log_box.pack(padx=20, pady=10, fill="both", expand=True)
        self.log_box.configure(state="disabled")

        # Start polling UI queue
        self.after(100, self._drain_ui_queue)

    def show_extractor(self):
        self.clear_content()
        ctk.CTkLabel(self.content, text="Unity Asset Bundle Extractor", font=("Segoe UI", 24)).pack(pady=20)

    # Vars
        self.bundle_file_var = ctk.StringVar()
        self.extract_output_var = ctk.StringVar()

        # Inputs
        ctk.CTkLabel(self.content, text="Unity Asset Bundle File:").pack(pady=5)
        ctk.CTkEntry(self.content, textvariable=self.bundle_file_var, corner_radius=50, width=500).pack(pady=5)
        ctk.CTkButton(self.content, text="Browse File", corner_radius=50, command=self.browse_asset_file).pack(pady=5)

        ctk.CTkLabel(self.content, text="Output Folder:").pack(pady=5)
        ctk.CTkEntry(self.content, textvariable=self.extract_output_var, corner_radius=50, width=500).pack(pady=5)
        ctk.CTkButton(self.content, text="Browse Folder", corner_radius=50, command=self.browse_extract_output).pack(pady=5)

    # (No advanced options)

        # Controls row
        btn_frame = ctk.CTkFrame(self.content)
        btn_frame.pack(pady=10)
        self.start_extract_btn = ctk.CTkButton(btn_frame, text="Start Extraction", corner_radius=50, command=self.start_extraction)
        self.start_extract_btn.pack(side="left", padx=5)
        self.cancel_extract_btn = ctk.CTkButton(btn_frame, text="Cancel", corner_radius=50, command=self.cancel_current, state="disabled")
        self.cancel_extract_btn.pack(side="left", padx=5)
        self.open_extract_btn = ctk.CTkButton(btn_frame, text="Open Output Folder", corner_radius=50, command=lambda: self.open_folder(self.extract_output_var.get()), state="disabled")
        self.open_extract_btn.pack(side="left", padx=5)

        # Progress + status
        self.ext_progress = ctk.CTkProgressBar(self.content)
        self.ext_progress.set(0)
        self.ext_progress.pack(pady=10, fill="x", padx=20)
        self.ext_status = ctk.CTkLabel(self.content, text="Idle")
        self.ext_status.pack(pady=5)

        # Log (share the same log box as converter for simplicity)
        self.log_box = ctk.CTkTextbox(self.content, height=200)
        self.log_box.pack(padx=20, pady=10, fill="both", expand=True)
        self.log_box.configure(state="disabled")

        # Start polling UI queue
        self.after(100, self._drain_ui_queue)

    # def show_settings(self):
    #     self.clear_content()
    #     ctk.CTkLabel(self.content, text="Settings", font=("Segoe UI", 24)).pack(pady=20)
    #     ctk.CTkButton(self.content, text="Example Setting 1", corner_radius=50).pack(pady=10)
    #     ctk.CTkButton(self.content, text="Example Setting 2", corner_radius=50).pack(pady=10)

    # ----------------- Utilities & Callbacks -----------------
    def log(self, msg: str):
        try:
            self.log_box.configure(state="normal")
            self.log_box.insert("end", msg + "\n")
            self.log_box.see("end")
            self.log_box.configure(state="disabled")
        except Exception:
            # Log box may not exist yet
            pass

    def _drain_ui_queue(self):
        # Drain queued UI updates posted from background threads
        try:
            while True:
                fn, args, kwargs = self._ui_queue.get_nowait()
                try:
                    fn(*args, **kwargs)
                except Exception:
                    pass
        except queue.Empty:
            pass
        # keep polling
        self.after(100, self._drain_ui_queue)

    def post_ui(self, fn, *args, **kwargs):
        self._ui_queue.put((fn, args, kwargs))

    def cancel_current(self):
        self._cancel_event.set()
        self.log("Cancellation requested...")

    def open_folder(self, path: str):
        if not path:
            return
        if os.path.isdir(path):
            try:
                if _is_windows():
                    os.startfile(path)
                else:
                    # Fallback for other OS (not expected here)
                    import subprocess
                    subprocess.Popen(["open" if sys.platform == "darwin" else "xdg-open", path])
            except Exception as e:
                messagebox.showerror("Error", f"Failed to open folder: {e}")

    # ----- Converter callbacks -----
    def browse_input_folder(self):
        folder_selected = filedialog.askdirectory(title="Select Input Folder Containing .gz.asset Files")
        if folder_selected:
            self.input_folder_var.set(folder_selected)

    def browse_output_folder(self):
        folder_selected = filedialog.askdirectory(title="Select Output Folder for .dat Files")
        if folder_selected:
            self.output_folder_var.set(folder_selected)

    def start_conversion(self):
        input_folder = self.input_folder_var.get().strip()
        output_folder = self.output_folder_var.get().strip()

        if not input_folder or not output_folder:
            messagebox.showerror("Error", "Please select both input and output folders.")
            return
        if not os.path.isdir(input_folder):
            messagebox.showerror("Error", "Input folder does not exist.")
            return
        os.makedirs(output_folder, exist_ok=True)

        # Reset state
        self._cancel_event.clear()
        self.conv_progress.set(0)
        self.conv_status.configure(text="Starting...")
        self.open_conv_btn.configure(state="disabled")
        self.cancel_conv_btn.configure(state="normal")
        self.start_conv_btn.configure(state="disabled")

        t = threading.Thread(target=self._convert_files_worker, args=(input_folder, output_folder), daemon=True)
        self._threads.append(t)
        t.start()

    def _convert_files_worker(self, input_folder: str, output_folder: str):
        try:
            files = [f for f in os.listdir(input_folder) if f.endswith(".gz.asset")]
            total = len(files)
            if total == 0:
                self.post_ui(messagebox.showwarning, "No Files", "No .gz.asset files found in the selected input folder.")
                self.post_ui(self.conv_status.configure, text="No files found")
                self.post_ui(self.cancel_conv_btn.configure, state="disabled")
                self.post_ui(self.start_conv_btn.configure, state="normal")
                return

            converted = 0
            errors = 0
            for idx, filename in enumerate(files, start=1):
                if self._cancel_event.is_set():
                    self.post_ui(self.conv_status.configure, text="Cancelled")
                    break
                input_file = os.path.join(input_folder, filename)
                output_file = os.path.join(output_folder, filename.replace(".gz.asset", ".dat"))
                try:
                    with gzip.open(input_file, "rb") as gz_file:
                        with open(output_file, "wb") as out_file:
                            shutil.copyfileobj(gz_file, out_file)
                    converted += 1
                    self.post_ui(self.log, f"Converted: {filename} -> {os.path.basename(output_file)}")
                except Exception as e:
                    errors += 1
                    self.post_ui(self.log, f"Error processing {filename}: {e}")

                self.post_ui(self.conv_progress.set, idx / total)
                self.post_ui(self.conv_status.configure, text=f"{idx}/{total} processed")

            if not self._cancel_event.is_set():
                self.post_ui(messagebox.showinfo, "Success", f"Conversion complete! {converted} files converted, {errors} errors.\nSaved in: {output_folder}")
                self.post_ui(self.conv_status.configure, text=f"Done: {converted} ok, {errors} errors")
                self.post_ui(self.open_conv_btn.configure, state="normal")
        finally:
            self.post_ui(self.cancel_conv_btn.configure, state="disabled")
            self.post_ui(self.start_conv_btn.configure, state="normal")

    # ----- Extractor callbacks -----
    def browse_asset_file(self):
        file_path = filedialog.askopenfilename(title="Select Unity Asset Bundle File", filetypes=[("Unity bundles", "*.unity3d *.asset *.*")])
        if file_path:
            self.bundle_file_var.set(file_path)

    def browse_extract_output(self):
        folder_selected = filedialog.askdirectory(title="Select Output Folder for Extracted Assets")
        if folder_selected:
            self.extract_output_var.set(folder_selected)

    def start_extraction(self):
        file_path = self.bundle_file_var.get().strip()
        output_directory = self.extract_output_var.get().strip()

        if not file_path:
            messagebox.showerror("Error", "Please select a Unity Asset Bundle file.")
            return
        if not os.path.isfile(file_path):
            messagebox.showerror("Error", "Selected bundle file does not exist.")
            return

        if not output_directory:
            # default to app/extracted_assets
            base_directory = os.path.dirname(sys.executable if getattr(sys, 'frozen', False) else os.path.abspath(__file__))
            output_directory = os.path.join(base_directory, "extracted_assets")
            self.extract_output_var.set(output_directory)

        os.makedirs(output_directory, exist_ok=True)

        # Reset state
        self._cancel_event.clear()
        self.ext_progress.set(0)
        self.ext_status.configure(text="Starting...")
        self.open_extract_btn.configure(state="disabled")
        self.cancel_extract_btn.configure(state="normal")
        self.start_extract_btn.configure(state="disabled")
        # No 'Extract From Raw Bundle' button to disable

        t = threading.Thread(target=self._extract_asset_bundle_worker, args=(file_path, output_directory), daemon=True)
        self._threads.append(t)
        t.start()

    # Removed start_extraction_from_raw as the button is no longer present

    def _extract_asset_bundle_worker(self, file_path: str, output_directory: str, is_raw: bool = False):
        raw_bundle_path = os.path.join(output_directory, "raw_asset_bundle.unity3d")
        try:
            try:
                if not is_raw:
                    # If input bundle is already the raw file in output dir, skip copy
                    if os.path.abspath(file_path) != os.path.abspath(raw_bundle_path):
                        shutil.copyfile(file_path, raw_bundle_path)
                        self.post_ui(self.log, f"Raw asset bundle saved to: {raw_bundle_path}")
                else:
                    self.post_ui(self.log, f"Using existing raw bundle: {raw_bundle_path}")
            except Exception as e:
                self.post_ui(messagebox.showerror, "Error", f"Failed to save the raw asset bundle: {e}")
                self.post_ui(self.ext_status.configure, text="Failed copying bundle")
                return

            # Only keep the raw bundle copy
            self.post_ui(messagebox.showinfo, "Success", f"Raw bundle saved to:\n{raw_bundle_path}")
            self.post_ui(self.ext_status.configure, text="Done (raw only)")
            self.post_ui(self.open_extract_btn.configure, state="normal")
        finally:
            self.post_ui(self.cancel_extract_btn.configure, state="disabled")
            self.post_ui(self.start_extract_btn.configure, state="normal")
            # No raw button to re-enable

# Run the application
if __name__ == "__main__":
    app = ModernUI()
    app.mainloop()
