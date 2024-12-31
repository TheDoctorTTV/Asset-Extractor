import os
import gzip
import shutil
import sys
import customtkinter as ctk
from tkinter import filedialog, messagebox, Canvas

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
        messagebox.showinfo("Info", "Unity Asset extraction is not available.")

# ----------------- GUI Setup -----------------
ctk.set_appearance_mode("System")
ctk.set_default_color_theme("blue")

class RoundedEntry(Canvas):
    def __init__(self, parent, textvariable=None, width=300, height=40, corner_radius=15, bg_color="#D3D3D3", **kwargs):
        super().__init__(parent, width=width, height=height, bg=bg_color, highlightthickness=0, **kwargs)
        self.corner_radius = corner_radius
        self.width = width
        self.height = height
        self.bg_color = bg_color

        self.create_rounded_rectangle(0, 0, width, height, fill=self.bg_color, outline="")
        self.entry = ctk.CTkEntry(self, textvariable=textvariable, width=width-20, height=height-10, fg_color="transparent")
        self.entry.place(x=10, y=5)

    def create_rounded_rectangle(self, x1, y1, x2, y2, **kwargs):
        radius = self.corner_radius
        self.create_arc(x1, y1, x1 + radius * 2, y1 + radius * 2, start=90, extent=90, **kwargs)
        self.create_arc(x2 - radius * 2, y1, x2, y1 + radius * 2, start=0, extent=90, **kwargs)
        self.create_arc(x2 - radius * 2, y2 - radius * 2, x2, y2, start=270, extent=90, **kwargs)
        self.create_arc(x1, y2 - radius * 2, x1 + radius * 2, y2, start=180, extent=90, **kwargs)
        self.create_rectangle(x1 + radius, y1, x2 - radius, y2, **kwargs)
        self.create_rectangle(x1, y1 + radius, x2, y2 - radius, **kwargs)

class ModernUI(ctk.CTk):
    def __init__(self):
        super().__init__()

        # Window properties
        self.title("Beat Saber Map Extractor")
        self.geometry("1024x768")
        self.resizable(False, False)

        # Header Frame
        self.header_frame = ctk.CTkFrame(self, height=80, corner_radius=15)
        self.header_frame.pack(side="top", fill="x", padx=10, pady=10)
        self.header_label = ctk.CTkLabel(self.header_frame, text="Beat Saber Map Extractor", font=("Segoe UI", 28))
        self.header_label.pack(pady=20)

        # Main layout
        self.main_frame = ctk.CTkFrame(self, corner_radius=15)
        self.main_frame.pack(fill="both", expand=True, padx=20, pady=10)
        self.main_frame.grid_columnconfigure(0, weight=1)
        self.main_frame.grid_columnconfigure(1, weight=4)

        # Sidebar
        self.sidebar = ctk.CTkFrame(self.main_frame, width=220, corner_radius=15)
        self.sidebar.grid(row=0, column=0, sticky="ns", padx=10, pady=10)

        self.extractor_button = ctk.CTkButton(self.sidebar, text="Extractor", height=60, width=180, corner_radius=20, command=self.show_extractor)
        self.extractor_button.pack(pady=20, padx=20)

        self.converter_button = ctk.CTkButton(self.sidebar, text="Converter", height=60, width=180, corner_radius=20, command=self.show_converter)
        self.converter_button.pack(pady=20, padx=20)

        self.version_label = ctk.CTkLabel(self.sidebar, text="v0.5.1", font=("Segoe UI", 12))
        self.version_label.pack(side="bottom", pady=10)

        # Content Frame
        self.content_frame = ctk.CTkFrame(self.main_frame, corner_radius=15)
        self.content_frame.grid(row=0, column=1, sticky="nsew", padx=10, pady=10)

        # Show initial content
        self.show_extractor()

    def clear_content(self):
        for widget in self.content_frame.winfo_children():
            widget.destroy()

    def show_extractor(self):
        self.clear_content()
        ctk.CTkLabel(self.content_frame, text="Unity Asset Bundle Extractor", font=("Segoe UI", 24)).pack(pady=40)
        ctk.CTkButton(self.content_frame, text="Browse Unity Asset File", height=50, width=220, corner_radius=20, command=browse_asset_file).pack(pady=20)

    def show_converter(self):
        self.clear_content()
        global input_folder_var, output_folder_var
        input_folder_var = ctk.StringVar()
        output_folder_var = ctk.StringVar()

        ctk.CTkLabel(self.content_frame, text="GZ.Asset to .DAT Converter", font=("Segoe UI", 24)).pack(pady=20)

        form_frame = ctk.CTkFrame(self.content_frame, corner_radius=15)
        form_frame.pack(pady=40, padx=20)

        ctk.CTkLabel(form_frame, text="Input Folder:", font=("Segoe UI", 16)).grid(row=0, column=0, padx=10, pady=10, sticky="w")
        RoundedEntry(form_frame, textvariable=input_folder_var, width=400, height=40).grid(row=0, column=1, padx=10, pady=10, sticky="w")
        ctk.CTkButton(form_frame, text="Browse", command=browse_input_folder, width=140, corner_radius=10).grid(row=0, column=2, padx=10, pady=10)

        ctk.CTkLabel(form_frame, text="Output Folder:", font=("Segoe UI", 16)).grid(row=1, column=0, padx=10, pady=10, sticky="w")
        RoundedEntry(form_frame, textvariable=output_folder_var, width=400, height=40).grid(row=1, column=1, padx=10, pady=10, sticky="w")
        ctk.CTkButton(form_frame, text="Browse", command=browse_output_folder, width=140, corner_radius=10).grid(row=1, column=2, padx=10, pady=10)

        ctk.CTkButton(self.content_frame, text="Start Conversion", height=60, width=220, corner_radius=20, command=start_conversion).pack(pady=20, side="bottom")

# Run the application
if __name__ == "__main__":
    app = ModernUI()
    app.mainloop()
