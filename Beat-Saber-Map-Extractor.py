import os
import gzip
import UnityPy
import shutil
import sys
import customtkinter as ctk
from tkinter import filedialog, messagebox


def resource_path(relative_path):
    """ Get absolute path to resource (works for dev & for PyInstaller) """
    if hasattr(sys, '_MEIPASS'):
        # PyInstaller extracts to _MEIPASS dir
        return os.path.join(sys._MEIPASS, relative_path)
    return os.path.join(os.path.abspath("."), relative_path)

# ----------------- GZ.Asset to .DAT Conversion -----------------
def convert_files(input_folder, output_folder):
    if not os.path.exists(output_folder):
        os.makedirs(output_folder, exist_ok=True)

    for filename in os.listdir(input_folder):
        if filename.endswith(".gz.asset"):
            input_file = os.path.join(input_folder, filename)
            output_file = os.path.join(output_folder, filename.replace(".gz.asset", ".dat"))

            try:
                with gzip.open(input_file, "rb") as gz_file:
                    with open(output_file, "wb") as out_file:
                        out_file.write(gz_file.read())
                print(f"Converted: {filename} -> {os.path.basename(output_file)}")
            except Exception as e:
                print(f"Error processing {filename}: {str(e)}")

    messagebox.showinfo("Success", f"Conversion complete! Files saved in: {output_folder}")

def browse_input_folder():
    folder_selected = filedialog.askdirectory(title="Select Input Folder Containing .gz.asset Files")
    input_folder_var.set(folder_selected)

def browse_output_folder():
    folder_selected = filedialog.askdirectory(title="Select Output Folder for .dat Files")
    output_folder_var.set(folder_selected)

def start_conversion():
    input_folder = input_folder_var.get()
    output_folder = output_folder_var.get()

    if not input_folder or not output_folder:
        messagebox.showerror("Error", "Please select both input and output folders.")
        return

    convert_files(input_folder, output_folder)

def browse_asset_file():
    file_path = filedialog.askopenfilename(title="Select Unity Asset Bundle File")
    if file_path:
        extract_asset_bundle(file_path)

def extract_asset_bundle(file_path):
    base_directory = os.path.dirname(sys.executable if getattr(sys, 'frozen', False) else os.path.abspath(__file__))
    output_directory = os.path.join(base_directory, "extracted_assets")
    raw_bundle_path = os.path.join(output_directory, "raw_asset_bundle.unity3d")
    os.makedirs(output_directory, exist_ok=True)

    try:
        shutil.copyfile(file_path, raw_bundle_path)
        print(f"Raw asset bundle saved to: {raw_bundle_path}")
    except Exception as e:
        messagebox.showerror("Error", f"Failed to save the raw asset bundle: {str(e)}")
        return

    try:
        env = UnityPy.load(file_path)
        print("Unity Asset Bundle loaded successfully.")

        for count, obj in enumerate(env.objects):
            try:
                data = obj.read()
                object_name = getattr(data, "name", f"Unnamed_Object_{count}")
                output_path = os.path.join(output_directory, f"{object_name}_{data.type.name}_{count}")

                if hasattr(data, "save"):
                    data.save(output_path)
                elif data.type.name == "TextAsset" and hasattr(data, "script"):
                    with open(f"{output_path}.txt", "w", encoding="utf-8") as f:
                        f.write(data.script)
            except Exception as e:
                print(f"Failed to process object {count}: {str(e)}")
    except Exception as e:
        messagebox.showerror("Error", f"Failed to extract assets: {str(e)}")

    messagebox.showinfo("Success", f"Extraction complete! Files saved to:\n{output_directory}")

# ----------------- GUI Setup -----------------
ctk.set_default_color_theme("blue")

class ModernUI(ctk.CTk):
    def __init__(self):
        super().__init__()
        
        icon_path = resource_path("beat-saber-map-extractor-icon.ico")
        self.iconbitmap(default=icon_path)


        # Window properties
        self.title("Beat Saber Mapping Extractor")
        
        self.geometry("1000x600")  # Starting size
        self.minsize(600, 400)    # Minimum size allowed
        self.resizable(True, True) # Allow resizing (default is True but explicit is good)


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
        self.version_label = ctk.CTkLabel(self.sidebar, text="v0.5", font=("Segoe UI", 10))
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
        
        global input_folder_var, output_folder_var
        input_folder_var = ctk.StringVar()
        output_folder_var = ctk.StringVar()

        ctk.CTkLabel(self.content, text="Input Folder:").pack(pady=5)
        ctk.CTkEntry(self.content, textvariable=input_folder_var, corner_radius=50, width=400).pack(pady=5)
        ctk.CTkButton(self.content, text="Browse", corner_radius=50, command=browse_input_folder).pack(pady=5)

        ctk.CTkLabel(self.content, text="Output Folder:").pack(pady=5)
        ctk.CTkEntry(self.content, textvariable=output_folder_var, corner_radius=50, width=400).pack(pady=5)
        ctk.CTkButton(self.content, text="Browse", corner_radius=50, command=browse_output_folder).pack(pady=5)
        
        ctk.CTkButton(self.content, text="Start Conversion", corner_radius=50, command=start_conversion).pack(pady=20)

    def show_extractor(self):
        self.clear_content()
        ctk.CTkLabel(self.content, text="Unity Asset Bundle Extractor", font=("Segoe UI", 24)).pack(pady=20)
        
        ctk.CTkButton(self.content, text="Browse Unity Asset File", corner_radius=50, command=browse_asset_file).pack(pady=10)

    # def show_settings(self):
    #     self.clear_content()
    #     ctk.CTkLabel(self.content, text="Settings", font=("Segoe UI", 24)).pack(pady=20)
    #     ctk.CTkButton(self.content, text="Example Setting 1", corner_radius=50).pack(pady=10)
    #     ctk.CTkButton(self.content, text="Example Setting 2", corner_radius=50).pack(pady=10)

# Run the application
if __name__ == "__main__":
    app = ModernUI()
    app.mainloop()
