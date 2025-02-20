#define readSize 20

volatile char *uart = (volatile char *)0x10000000;

void println(char *string);
void print(char *string);
char *readln();
int main()
{
    char *string = readln();
    print(string);
    println(" is cool");
    return 0;
}

void print(char *string)
{
    int i = 0;
    while (string[i] != '\0')
    {
        uart[0] = string[i];
        i++;
    }
}

void println(char *string)
{
    print(string);
    uart[0] = '\n';
}

char *readln()
{
    static char buffer[readSize];
    char letter;
    int i = 0;
    while (i < readSize)
    {
        // polling uart for next letter
        while ((uart[5] & 0x01) == 0)
            ;
        letter = uart[0];
        if (letter != '\n')
        {
            buffer[i] = letter;
        }
        else
        {
            break;
        }
        i++;
    }

    return (char *)&buffer;
}