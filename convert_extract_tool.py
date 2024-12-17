import os
import gzip
import UnityPy
import shutil
import sys
import customtkinter as ctk
from tkinter import filedialog, messagebox, Menu  # Standard tkinter Menu

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

# ----------------- Unity Asset Bundle Extraction -----------------
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

def browse_asset_file():
    file_path = filedialog.askopenfilename(title="Select Unity Asset Bundle File")
    if file_path:
        extract_asset_bundle(file_path)

# ----------------- GUI Setup -----------------
ctk.set_appearance_mode("Dark")  # Set dark mode
ctk.set_default_color_theme("blue")  # Default theme color

root = ctk.CTk()
root.title("GZ.Asset Converter & Unity Asset Extractor")
root.geometry("700x500")

# Input/Output Folder Variables
input_folder_var = ctk.StringVar()
output_folder_var = ctk.StringVar()

# Tabs
tabview = ctk.CTkTabview(root)
tabview.pack(expand=True, fill="both", padx=10, pady=10)

# Tab 1: GZ.Asset to .DAT Converter
tab1 = tabview.add("GZ.Asset to .DAT")

ctk.CTkLabel(tab1, text="Input Folder:", font=("Segoe UI", 14)).pack(pady=5)
ctk.CTkEntry(tab1, textvariable=input_folder_var, width=400).pack(pady=5)
ctk.CTkButton(tab1, text="Browse", command=browse_input_folder).pack(pady=5)

ctk.CTkLabel(tab1, text="Output Folder:", font=("Segoe UI", 14)).pack(pady=5)
ctk.CTkEntry(tab1, textvariable=output_folder_var, width=400).pack(pady=5)
ctk.CTkButton(tab1, text="Browse", command=browse_output_folder).pack(pady=5)

ctk.CTkButton(tab1, text="Start Conversion", fg_color="green", command=start_conversion).pack(pady=20)

# Tab 2: Unity Asset Bundle Extractor
tab2 = tabview.add("Unity Asset Extractor")

ctk.CTkLabel(tab2, text="Unity Asset Bundle Extractor", font=("Segoe UI", 18)).pack(pady=10)
ctk.CTkButton(tab2, text="Browse Unity Asset File", command=browse_asset_file).pack(pady=20)

# Menu Bar Setup (Standard Tkinter Menu)
menu = Menu(root)
root.config(menu=menu)

file_menu = Menu(menu, tearoff=0)
menu.add_cascade(label="File", menu=file_menu)
file_menu.add_command(label="Exit", command=root.quit)

help_menu = Menu(menu, tearoff=0)
menu.add_cascade(label="Help", menu=help_menu)
help_menu.add_command(label="About", command=lambda: messagebox.showinfo("About", "Unity Asset Extractor & Converter\nVersion 0.3.1 Alpha\nMade by TheDoctorTTV"))

# Start the application
root.mainloop()
