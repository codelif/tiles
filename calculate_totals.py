import csv

def calculate_total_donations(file_path):
    total_payments = 0.0
    total_payouts = 0.0
    
    try:
        with open(file_path, mode='r', encoding='utf-8') as csvfile:
            reader = csv.DictReader(csvfile)
            for row in reader:
                amount = float(row['Amount'])
                if row['Type'] == 'payment':
                    total_payments += amount
                elif row['Type'] == 'payout':
                    total_payouts += amount
                    
        print(f"Total Payments (Donations): {total_payments:,.2f}")
        print(f"Total Payouts: {total_payouts:,.2f}")
        print(f"Grand Total: {total_payments + total_payouts:,.2f}")
        
    except FileNotFoundError:
        print(f"Error: File not found at {file_path}")
    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    file_path = '/home/primalpimmy/Prefix/balance_history.csv'
    calculate_total_donations(file_path)
