from flask import Flask, render_template, request, jsonify
import os
import gzip
from werkzeug.utils import secure_filename
import magic  # For MIME type detection
import shutil  # For file copying
import threading
import webbrowser
import customtkinter as ctk
from tkinter import messagebox 

app = Flask(__name__)

# Set folders
UPLOAD_FOLDER = "uploads"
CONVERT_FOLDER = "converted"
EXTRACT_FOLDER = "extracted"
os.makedirs(UPLOAD_FOLDER, exist_ok=True)
os.makedirs(CONVERT_FOLDER, exist_ok=True)
os.makedirs(EXTRACT_FOLDER, exist_ok=True)

app.config['UPLOAD_FOLDER'] = UPLOAD_FOLDER

# Utility function to check allowed file extensions
def allowed_file(filename, extensions):
    return '.' in filename and filename.rsplit('.', 1)[1].lower() in extensions

# Home route
@app.route('/')
def home():
    return render_template("index.html")

# Converter page route
@app.route('/converter')
def converter():
    return render_template("converter.html")

# Extractor page route
@app.route('/extractor')
def extractor():
    return render_template("extractor.html")

# Conversion logic
@app.route('/convert', methods=['POST'])
def convert_files():
    uploaded_files = request.files.getlist("input_files")
    converted_files = []
    output_folder = os.path.join(CONVERT_FOLDER, "converted")
    os.makedirs(output_folder, exist_ok=True)

    for file in uploaded_files:
        if file.filename.endswith(".gz.asset"):
            filename = secure_filename(file.filename)
            input_path = os.path.join(UPLOAD_FOLDER, filename)
            output_path = os.path.join(output_folder, filename.replace(".gz.asset", ".dat"))
            file.save(input_path)

            try:
                # Decompress .gz.asset and save as .dat
                with gzip.open(input_path, "rb") as gz_file:
                    with open(output_path, "wb") as out_file:
                        out_file.write(gz_file.read())
                converted_files.append(output_path)
            except Exception as e:
                return jsonify({"error": f"Failed to convert {file.filename}: {str(e)}"}), 500

    return jsonify({"message": "Conversion complete!", "files": converted_files})

# Extraction logic
@app.route('/extract', methods=['POST'])
def extract_files():
    uploaded_file = request.files.get("asset_file")
    if not uploaded_file:
        return jsonify({"error": "No file uploaded."}), 400

    filename = secure_filename(uploaded_file.filename)
    input_path = os.path.join(UPLOAD_FOLDER, filename)
    output_folder = os.path.join(EXTRACT_FOLDER, filename.split('.')[0])
    os.makedirs(output_folder, exist_ok=True)
    uploaded_file.save(input_path)

    try:
        output_file_path = os.path.join(output_folder, "assetbundle.unity3d")
        shutil.copy(input_path, output_file_path)  # Placeholder: Simulate extraction
    except Exception as e:
        return jsonify({"error": f"Failed to extract file: {str(e)}"}), 500

    return jsonify({"message": "Extraction complete!", "files": [output_file_path]})


def start_server():
    app.run(debug=False, port=5500)

def create_ui():
    # Initialize the CustomTkinter UI
    ctk.set_appearance_mode("System")  # Options: "System" (default), "Dark", "Light"
    ctk.set_default_color_theme("blue")  # Options: "blue" (default), "green", "dark-blue"

    root = ctk.CTk()
    root.title("WebServer")
    root.geometry("300x150")
    root.resizable(False, False)

    def open_link():
        webbrowser.open("http://127.0.0.1:5500")

    def on_close():
        if messagebox.askokcancel("Quit", "Do you want to stop the server and quit?"):
            root.destroy()
            os._exit(0)  # Ensure the Flask server also shuts down

    ctk.CTkLabel(root, text="Webserver is running!", font=("Arial", 14)).pack(pady=10)
    ctk.CTkButton(root, text="Open Web Interface", command=open_link).pack(pady=10)
    ctk.CTkButton(root, text="Quit", command=on_close).pack(pady=10)

    root.protocol("WM_DELETE_WINDOW", on_close)
    root.mainloop()


if __name__ == "__main__":
    # Start the Flask server in a separate thread
    threading.Thread(target=start_server, daemon=True).start()

    # Start the CustomTkinter UI
    create_ui()
