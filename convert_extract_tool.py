import os
import gzip
import UnityPy
import shutil
from datetime import datetime
import tkinter as tk
from tkinter import ttk, filedialog, messagebox

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
    # Determine output directory next to the executable
    if getattr(sys, 'frozen', False):
        base_directory = os.path.dirname(sys.executable)
    else:
        base_directory = os.path.dirname(os.path.abspath(__file__))
    
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

        object_count = 0
        for obj in env.objects:
            try:
                data = obj.read()
                object_name = getattr(data, "name", f"Unnamed_Object_{object_count}")
                output_path = os.path.join(output_directory, f"{object_name}_{data.type.name}_{object_count}")

                if hasattr(data, "save"):
                    data.save(output_path)
                elif data.type.name == "TextAsset" and hasattr(data, "script"):
                    with open(f"{output_path}.txt", "w", encoding="utf-8") as f:
                        f.write(data.script)
            except Exception as e:
                print(f"Failed to process object {object_count}: {str(e)}")
            object_count += 1
    except Exception as e:
        messagebox.showerror("Error", f"Failed to extract assets: {str(e)}")
        return

    messagebox.showinfo("Success", f"Extraction complete! Files saved to:\n{output_directory}")


def browse_asset_file():
    file_path = filedialog.askopenfilename(title="Select Unity Asset Bundle File")
    if file_path:
        extract_asset_bundle(file_path)

# ----------------- GUI Setup -----------------
root = tk.Tk()
root.title("GZ.Asset Converter & Unity Asset Extractor")
root.geometry("500x400")

notebook = ttk.Notebook(root)

# --- Tab 1: GZ.Asset to .DAT Converter ---
tab1 = ttk.Frame(notebook)
notebook.add(tab1, text="GZ.Asset to .DAT")

input_folder_var = tk.StringVar()
output_folder_var = tk.StringVar()

ttk.Label(tab1, text="Input Folder:").pack(pady=5)
ttk.Entry(tab1, textvariable=input_folder_var, width=50).pack(pady=5)
ttk.Button(tab1, text="Browse", command=browse_input_folder).pack(pady=5)

ttk.Label(tab1, text="Output Folder:").pack(pady=5)
ttk.Entry(tab1, textvariable=output_folder_var, width=50).pack(pady=5)
ttk.Button(tab1, text="Browse", command=browse_output_folder).pack(pady=5)

ttk.Button(tab1, text="Start Conversion", command=start_conversion).pack(pady=10)

# --- Tab 2: Unity Asset Bundle Extractor ---
tab2 = ttk.Frame(notebook)
notebook.add(tab2, text="Unity Asset Extractor")

ttk.Label(tab2, text="Unity Asset Bundle Extractor", font=("Arial", 14)).pack(pady=10)
ttk.Button(tab2, text="Browse Unity Asset File", command=browse_asset_file).pack(pady=20)

# Finalizing GUI
notebook.pack(expand=1, fill="both")
root.mainloop()
